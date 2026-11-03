//! Interface-Bound Multi-Path HTTP/1.1 & HTTPS Range Downloader (`MultiPathDownloader`).
//!
//! Uses `giga_pal::connect_bound_tcp_v4` (`IP_UNICAST_IF` + `bind()`) for raw HTTP range streaming
//! and Windows native Schannel TLS (`curl.exe --interface <local_ipv4>`) for HTTPS range streaming,
//! ensuring zero external TLS C-library build dependencies while binding every range worker to its
//! assigned network adapter.

use giga_inr::InrEngine;
use giga_pal::{connect_bound_tcp_v4, InterfaceSnapshot};
use giga_scheduler::{ByteRangeChunk, MultiPathScheduler};
use giga_store::{
    compute_file_sha256_hex, RandomAccessFileAssembler, TransferManifest, TransferSessionState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUrl {
    pub is_tls: bool,
    pub host: String,
    pub port: u16,
    pub path_and_query: String,
}

impl ParsedUrl {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        let (is_tls, rest, default_port) = if let Some(r) = trimmed.strip_prefix("https://") {
            (true, r, 443u16)
        } else if let Some(r) = trimmed.strip_prefix("http://") {
            (false, r, 80u16)
        } else {
            return Err(format!(
                "URL '{}' must start with http:// or https://",
                trimmed
            ));
        };

        let (authority, path_and_query) = match rest.find('/') {
            Some(idx) => (&rest[..idx], rest[idx..].to_string()),
            None => (rest, "/".to_string()),
        };

        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => (
                h.to_string(),
                p.parse::<u16>().map_err(|_| "Invalid port in URL")?,
            ),
            None => (authority.to_string(), default_port),
        };

        if host.is_empty() {
            return Err("URL host cannot be empty".to_string());
        }

        Ok(Self {
            is_tls,
            host,
            port,
            path_and_query,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressSnapshot {
    pub url: String,
    pub output_path: String,
    pub state: TransferSessionState,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub progress_pct: f64,
    pub aggregate_speed_mbps: f64,
    pub elapsed_sec: f64,
    pub active_chunks: usize,
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub work_stolen_chunks: usize,
    pub per_interface_bytes: HashMap<u32, u64>,
    pub sha256_hex: Option<String>,
}

#[derive(Clone)]
pub struct MultiPathDownloader {
    inr: InrEngine,
    latest_progress: Arc<Mutex<Option<DownloadProgressSnapshot>>>,
}

impl MultiPathDownloader {
    pub fn new(inr: InrEngine) -> Self {
        Self {
            inr,
            latest_progress: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn latest_progress(&self) -> Option<DownloadProgressSnapshot> {
        self.latest_progress.lock().await.clone()
    }

    /// Executes a full multi-interface striped download of `url` into `output_path`,
    /// returning the final [`DownloadProgressSnapshot`] with verified SHA-256 hash.
    pub async fn download_url(
        &self,
        url: &str,
        output_path: &Path,
        workers_per_interface: usize,
    ) -> Result<DownloadProgressSnapshot, String> {
        let parsed = ParsedUrl::parse(url)?;
        let mut ranked = self.inr.ranked_healthy_interfaces().await;
        if ranked.is_empty() {
            let _ = self.inr.refresh_and_probe(Duration::from_millis(2500)).await;
            ranked = self.inr.ranked_healthy_interfaces().await;
        }
        if ranked.is_empty() {
            return Err("No healthy WAN interfaces found for download".to_string());
        }

        let raw_ifaces: Vec<InterfaceSnapshot> = ranked.iter().map(|(i, _)| i.clone()).collect();
        let dns_res = self
            .inr
            .dns()
            .resolve_v4(&parsed.host, &raw_ifaces, Duration::from_millis(3000))
            .await?;
        let target_ip = *dns_res
            .resolved_ips
            .first()
            .ok_or_else(|| "DNS returned no IPv4 addresses".to_string())?;
        let remote_addr = SocketAddr::new(IpAddr::V4(target_ip), parsed.port);

        let primary_iface = &ranked[0].0;
        let (total_bytes, supports_range) = self
            .probe_remote_resource(primary_iface, remote_addr, &parsed, url)
            .await?;

        let start_instant = Instant::now();

        if !supports_range || total_bytes == 0 {
            return self
                .download_single_stream(
                    primary_iface,
                    remote_addr,
                    &parsed,
                    url,
                    output_path,
                    start_instant,
                )
                .await;
        }

        let scheduler = Arc::new(Mutex::new(MultiPathScheduler::new(
            total_bytes,
            raw_ifaces.len(),
        )));
        let assembler = RandomAccessFileAssembler::open_or_create(output_path, total_bytes, false)
            .map_err(|e| format!("Failed to open output file: {}", e))?;

        let per_if_bytes: Arc<Mutex<HashMap<u32, u64>>> = Arc::new(Mutex::new(HashMap::new()));
        let workers_per_if = workers_per_interface.clamp(1, 8);

        let mut worker_handles = Vec::new();
        for iface in &raw_ifaces {
            for _ in 0..workers_per_if {
                let dl = self.clone();
                let iface_clone = iface.clone();
                let parsed_clone = parsed.clone();
                let sched_clone = scheduler.clone();
                let asm_clone = assembler.clone();
                let per_if_clone = per_if_bytes.clone();
                let url_str = url.to_string();
                let out_str = output_path.display().to_string();

                worker_handles.push(tokio::spawn(async move {
                    loop {
                        let claimed: Option<ByteRangeChunk> = {
                            let mut s = sched_clone.lock().await;
                            s.claim_or_steal_chunk(iface_clone.if_index)
                        };
                        let Some(chunk) = claimed else {
                            break;
                        };

                        let res = dl
                            .execute_chunk_worker(
                                &iface_clone,
                                remote_addr,
                                &parsed_clone,
                                chunk.clone(),
                                &sched_clone,
                                &asm_clone,
                                &per_if_clone,
                                &url_str,
                                &out_str,
                                start_instant,
                            )
                            .await;

                        if res.is_err() {
                            let mut s = sched_clone.lock().await;
                            s.mark_chunk_failed(chunk.chunk_id);
                            tokio::time::sleep(Duration::from_millis(150)).await;
                        }
                    }
                }));
            }
        }

        for h in worker_handles {
            let _ = h.await;
        }

        assembler
            .sync_all()
            .map_err(|e| format!("Failed to flush output file: {}", e))?;

        let sha256_hex = compute_file_sha256_hex(output_path).ok();
        TransferManifest::remove_journal(output_path);

        let elapsed_sec = start_instant.elapsed().as_secs_f64().max(0.001);
        let sched_guard = scheduler.lock().await;
        let downloaded_bytes = sched_guard.total_downloaded_bytes();
        let completed_chunks = sched_guard
            .chunks
            .iter()
            .filter(|c| c.state == giga_scheduler::ChunkState::Completed)
            .count();
        let stolen_chunks = sched_guard
            .chunks
            .iter()
            .filter(|c| c.stolen_from_chunk.is_some())
            .count();
        let total_chunks = sched_guard.chunks.len();
        drop(sched_guard);

        let if_map = per_if_bytes.lock().await.clone();
        let final_snap = DownloadProgressSnapshot {
            url: url.to_string(),
            output_path: output_path.display().to_string(),
            state: if downloaded_bytes >= total_bytes {
                TransferSessionState::Completed
            } else {
                TransferSessionState::Failed
            },
            total_bytes,
            downloaded_bytes,
            progress_pct: (downloaded_bytes as f64 / total_bytes as f64 * 100.0).clamp(0.0, 100.0),
            aggregate_speed_mbps: (downloaded_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0),
            elapsed_sec,
            active_chunks: 0,
            completed_chunks,
            total_chunks,
            work_stolen_chunks: stolen_chunks,
            per_interface_bytes: if_map,
            sha256_hex,
        };

        *self.latest_progress.lock().await = Some(final_snap.clone());
        Ok(final_snap)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_chunk_worker(
        &self,
        iface: &InterfaceSnapshot,
        remote_addr: SocketAddr,
        parsed: &ParsedUrl,
        chunk: ByteRangeChunk,
        scheduler: &Arc<Mutex<MultiPathScheduler>>,
        assembler: &RandomAccessFileAssembler,
        per_if_bytes: &Arc<Mutex<HashMap<u32, u64>>>,
        url_str: &str,
        out_str: &str,
        start_instant: Instant,
    ) -> Result<(), String> {
        if parsed.is_tls {
            return self
                .execute_tls_chunk_worker(
                    iface,
                    chunk,
                    scheduler,
                    assembler,
                    per_if_bytes,
                    url_str,
                    out_str,
                    start_instant,
                )
                .await;
        }

        let timeout = Duration::from_millis(6000);
        let (mut tcp_stream, conn_info) = connect_bound_tcp_v4(iface, remote_addr, timeout)
            .await
            .map_err(|e| e.to_string())?;

        self.inr
            .record_transfer_sample(iface.if_index, 0, 0, Some(conn_info.handshake_rtt_ms), true)
            .await;

        let req_header = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: WiFiGigaChad420/0.1\r\nRange: bytes={}-{}\r\nConnection: close\r\n\r\n",
            parsed.path_and_query, parsed.host, chunk.current_byte, chunk.end_byte
        );

        tcp_stream
            .write_all(req_header.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        let (status_code, initial_body) = read_http_headers(&mut tcp_stream).await?;
        if status_code != 206 && status_code != 200 {
            return Err(format!("Unexpected HTTP status {} on range chunk", status_code));
        }

        let mut write_offset = chunk.current_byte;
        let mut window_bytes = 0u64;
        let mut window_start = Instant::now();

        if !initial_body.is_empty() {
            let effective_end = {
                let s = scheduler.lock().await;
                s.chunks
                    .iter()
                    .find(|c| c.chunk_id == chunk.chunk_id)
                    .map(|c| c.end_byte)
                    .unwrap_or(chunk.end_byte)
            };
            let max_writable = (effective_end + 1).saturating_sub(write_offset) as usize;
            let to_write = initial_body.len().min(max_writable);
            if to_write > 0 {
                assembler
                    .write_at(write_offset, &initial_body[..to_write])
                    .map_err(|e| e.to_string())?;
                write_offset += to_write as u64;
                window_bytes += to_write as u64;
                let mut s = scheduler.lock().await;
                s.advance_chunk(chunk.chunk_id, to_write as u64);
                *per_if_bytes.lock().await.entry(iface.if_index).or_insert(0) += to_write as u64;
            }
        }

        let mut buf = [0u8; 32768];
        loop {
            let current_end = {
                let s = scheduler.lock().await;
                let Some(c) = s.chunks.iter().find(|c| c.chunk_id == chunk.chunk_id) else {
                    break;
                };
                if c.state == giga_scheduler::ChunkState::Completed || write_offset > c.end_byte {
                    break;
                }
                c.end_byte
            };

            let n = tokio::time::timeout(timeout, tcp_stream.read(&mut buf))
                .await
                .map_err(|_| "Chunk socket read timeout".to_string())?
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }

            let max_writable = (current_end + 1).saturating_sub(write_offset) as usize;
            let to_write = n.min(max_writable);
            if to_write > 0 {
                assembler
                    .write_at(write_offset, &buf[..to_write])
                    .map_err(|e| e.to_string())?;
                write_offset += to_write as u64;
                window_bytes += to_write as u64;

                let (_, _, completed) = {
                    let mut s = scheduler.lock().await;
                    let res = s
                        .advance_chunk(chunk.chunk_id, to_write as u64)
                        .unwrap_or((write_offset, current_end, true));
                    let dt = window_start.elapsed().as_secs_f64();
                    if dt >= 0.25 {
                        let bps = (window_bytes as f64 * 8.0) / dt;
                        s.record_bandwidth(iface.if_index, bps, dt);
                    }
                    res
                };

                *per_if_bytes.lock().await.entry(iface.if_index).or_insert(0) += to_write as u64;

                let dt = window_start.elapsed().as_secs_f64();
                if dt >= 0.25 {
                    self.inr
                        .record_transfer_sample(iface.if_index, window_bytes, 0, None, true)
                        .await;
                    window_bytes = 0;
                    window_start = Instant::now();
                    self.publish_live_progress(url_str, out_str, scheduler, per_if_bytes, start_instant)
                        .await;
                }

                if completed {
                    break;
                }
            } else {
                break;
            }
        }

        if window_bytes > 0 {
            self.inr
                .record_transfer_sample(iface.if_index, window_bytes, 0, None, true)
                .await;
        }
        self.publish_live_progress(url_str, out_str, scheduler, per_if_bytes, start_instant)
            .await;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_tls_chunk_worker(
        &self,
        iface: &InterfaceSnapshot,
        chunk: ByteRangeChunk,
        scheduler: &Arc<Mutex<MultiPathScheduler>>,
        assembler: &RandomAccessFileAssembler,
        per_if_bytes: &Arc<Mutex<HashMap<u32, u64>>>,
        url_str: &str,
        out_str: &str,
        start_instant: Instant,
    ) -> Result<(), String> {
        let local_ip = iface
            .primary_ipv4()
            .ok_or_else(|| "Interface has no IPv4".to_string())?
            .to_string();
        let range_arg = format!("{}-{}", chunk.current_byte, chunk.end_byte);

        let mut child = Command::new("curl.exe")
            .args([
                "-sS",
                "-L",
                "--interface",
                &local_ip,
                "-r",
                &range_arg,
                url_str,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn bound TLS worker: {}", e))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Missing stdout pipe".to_string())?;

        let mut write_offset = chunk.current_byte;
        let mut window_bytes = 0u64;
        let mut window_start = Instant::now();
        let mut buf = [0u8; 32768];

        loop {
            let current_end = {
                let s = scheduler.lock().await;
                let Some(c) = s.chunks.iter().find(|c| c.chunk_id == chunk.chunk_id) else {
                    let _ = child.kill().await;
                    break;
                };
                if c.state == giga_scheduler::ChunkState::Completed || write_offset > c.end_byte {
                    let _ = child.kill().await;
                    break;
                }
                c.end_byte
            };

            let n = stdout.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }

            let max_writable = (current_end + 1).saturating_sub(write_offset) as usize;
            let to_write = n.min(max_writable);
            if to_write > 0 {
                assembler
                    .write_at(write_offset, &buf[..to_write])
                    .map_err(|e| e.to_string())?;
                write_offset += to_write as u64;
                window_bytes += to_write as u64;

                let (_, _, completed) = {
                    let mut s = scheduler.lock().await;
                    let res = s
                        .advance_chunk(chunk.chunk_id, to_write as u64)
                        .unwrap_or((write_offset, current_end, true));
                    let dt = window_start.elapsed().as_secs_f64();
                    if dt >= 0.25 {
                        let bps = (window_bytes as f64 * 8.0) / dt;
                        s.record_bandwidth(iface.if_index, bps, dt);
                    }
                    res
                };

                *per_if_bytes.lock().await.entry(iface.if_index).or_insert(0) += to_write as u64;

                let dt = window_start.elapsed().as_secs_f64();
                if dt >= 0.25 {
                    self.inr
                        .record_transfer_sample(iface.if_index, window_bytes, 0, None, true)
                        .await;
                    window_bytes = 0;
                    window_start = Instant::now();
                    self.publish_live_progress(url_str, out_str, scheduler, per_if_bytes, start_instant)
                        .await;
                }

                if completed {
                    let _ = child.kill().await;
                    break;
                }
            } else {
                let _ = child.kill().await;
                break;
            }
        }

        if window_bytes > 0 {
            self.inr
                .record_transfer_sample(iface.if_index, window_bytes, 0, None, true)
                .await;
        }
        self.publish_live_progress(url_str, out_str, scheduler, per_if_bytes, start_instant)
            .await;

        Ok(())
    }

    async fn publish_live_progress(
        &self,
        url_str: &str,
        out_str: &str,
        scheduler: &Arc<Mutex<MultiPathScheduler>>,
        per_if_bytes: &Arc<Mutex<HashMap<u32, u64>>>,
        start_instant: Instant,
    ) {
        let s = scheduler.lock().await;
        let total_bytes = s.total_bytes.max(1);
        let downloaded_bytes = s.total_downloaded_bytes();
        let active_chunks = s
            .chunks
            .iter()
            .filter(|c| c.state == giga_scheduler::ChunkState::InFlight)
            .count();
        let completed_chunks = s
            .chunks
            .iter()
            .filter(|c| c.state == giga_scheduler::ChunkState::Completed)
            .count();
        let stolen_chunks = s
            .chunks
            .iter()
            .filter(|c| c.stolen_from_chunk.is_some())
            .count();
        let total_chunks = s.chunks.len();
        drop(s);

        let elapsed_sec = start_instant.elapsed().as_secs_f64().max(0.001);
        let snap = DownloadProgressSnapshot {
            url: url_str.to_string(),
            output_path: out_str.to_string(),
            state: TransferSessionState::Active,
            total_bytes,
            downloaded_bytes,
            progress_pct: (downloaded_bytes as f64 / total_bytes as f64 * 100.0).clamp(0.0, 100.0),
            aggregate_speed_mbps: (downloaded_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0),
            elapsed_sec,
            active_chunks,
            completed_chunks,
            total_chunks,
            work_stolen_chunks: stolen_chunks,
            per_interface_bytes: per_if_bytes.lock().await.clone(),
            sha256_hex: None,
        };
        *self.latest_progress.lock().await = Some(snap);
    }

    async fn probe_remote_resource(
        &self,
        iface: &InterfaceSnapshot,
        remote_addr: SocketAddr,
        parsed: &ParsedUrl,
        raw_url: &str,
    ) -> Result<(u64, bool), String> {
        if parsed.is_tls {
            let local_ip = iface
                .primary_ipv4()
                .ok_or_else(|| "Interface has no IPv4".to_string())?
                .to_string();
            let out = Command::new("curl.exe")
                .args([
                    "-s",
                    "-D",
                    "-",
                    "-o",
                    "NUL",
                    "-L",
                    "--interface",
                    &local_ip,
                    "-r",
                    "0-0",
                    raw_url,
                ])
                .output()
                .await
                .map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&out.stdout);
            return Ok(parse_probe_headers(&text));
        }

        let timeout = Duration::from_millis(5000);
        let (mut tcp_stream, info) = connect_bound_tcp_v4(iface, remote_addr, timeout)
            .await
            .map_err(|e| e.to_string())?;
        self.inr
            .record_transfer_sample(iface.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
            .await;

        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: WiFiGigaChad420/0.1\r\nRange: bytes=0-0\r\nConnection: close\r\n\r\n",
            parsed.path_and_query, parsed.host
        );

        tcp_stream
            .write_all(req.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        let mut buf = [0u8; 4096];
        let n = tokio::time::timeout(timeout, tcp_stream.read(&mut buf))
            .await
            .map_err(|_| "Probe read timeout".to_string())?
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&buf[..n]);
        let headers_part = text.split("\r\n\r\n").next().unwrap_or(&text);

        Ok(parse_probe_headers(headers_part))
    }

    async fn download_single_stream(
        &self,
        iface: &InterfaceSnapshot,
        remote_addr: SocketAddr,
        parsed: &ParsedUrl,
        url: &str,
        output_path: &Path,
        start_instant: Instant,
    ) -> Result<DownloadProgressSnapshot, String> {
        let assembler = RandomAccessFileAssembler::open_or_create(output_path, 0, false)
            .map_err(|e| e.to_string())?;
        let mut offset = 0u64;

        if parsed.is_tls {
            let local_ip = iface
                .primary_ipv4()
                .ok_or_else(|| "Interface has no IPv4".to_string())?
                .to_string();
            let mut child = Command::new("curl.exe")
                .args(["-sS", "-L", "--interface", &local_ip, url])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            let mut stdout = child.stdout.take().ok_or("Missing stdout")?;
            let mut buf = [0u8; 32768];
            loop {
                let n = stdout.read(&mut buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                assembler.write_at(offset, &buf[..n]).map_err(|e| e.to_string())?;
                offset += n as u64;
                self.inr
                    .record_transfer_sample(iface.if_index, n as u64, 0, None, true)
                    .await;
            }
        } else {
            let timeout = Duration::from_millis(6000);
            let (mut tcp_stream, _) = connect_bound_tcp_v4(iface, remote_addr, timeout)
                .await
                .map_err(|e| e.to_string())?;
            let req = format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: WiFiGigaChad420/0.1\r\nConnection: close\r\n\r\n",
                parsed.path_and_query, parsed.host
            );
            tcp_stream.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
            let (_, initial_body) = read_http_headers(&mut tcp_stream).await?;
            if !initial_body.is_empty() {
                assembler.write_at(offset, &initial_body).map_err(|e| e.to_string())?;
                offset += initial_body.len() as u64;
            }
            let mut buf = [0u8; 32768];
            loop {
                let n = tokio::time::timeout(timeout, tcp_stream.read(&mut buf))
                    .await
                    .map_err(|_| "Read timeout".to_string())?
                    .map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                assembler.write_at(offset, &buf[..n]).map_err(|e| e.to_string())?;
                offset += n as u64;
                self.inr
                    .record_transfer_sample(iface.if_index, n as u64, 0, None, true)
                    .await;
            }
        }

        let _ = assembler.sync_all();
        let sha256_hex = compute_file_sha256_hex(output_path).ok();
        let elapsed_sec = start_instant.elapsed().as_secs_f64().max(0.001);
        let mut map = HashMap::new();
        map.insert(iface.if_index, offset);

        Ok(DownloadProgressSnapshot {
            url: url.to_string(),
            output_path: output_path.display().to_string(),
            state: TransferSessionState::Completed,
            total_bytes: offset,
            downloaded_bytes: offset,
            progress_pct: 100.0,
            aggregate_speed_mbps: (offset as f64 * 8.0) / (elapsed_sec * 1_000_000.0),
            elapsed_sec,
            active_chunks: 0,
            completed_chunks: 1,
            total_chunks: 1,
            work_stolen_chunks: 0,
            per_interface_bytes: map,
            sha256_hex,
        })
    }
}

fn parse_probe_headers(headers_part: &str) -> (u64, bool) {
    let mut total_bytes = 0u64;
    let mut supports_range = false;

    for line in headers_part.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("http/") && lower.contains(" 206 ") {
            supports_range = true;
        } else if let Some(cr) = lower.strip_prefix("content-range:") {
            supports_range = true;
            if let Some((_, total_str)) = cr.trim().rsplit_once('/') {
                if let Ok(tb) = total_str.trim().parse::<u64>() {
                    total_bytes = tb;
                }
            }
        } else if let Some(cl) = lower.strip_prefix("content-length:") {
            if total_bytes == 0 {
                if let Ok(tb) = cl.trim().parse::<u64>() {
                    if tb > 1 {
                        total_bytes = tb;
                    }
                }
            }
        } else if lower.starts_with("accept-ranges:") && lower.contains("bytes") {
            supports_range = true;
        }
    }
    (total_bytes, supports_range)
}

async fn read_http_headers(
    stream: &mut tokio::net::TcpStream,
) -> Result<(u16, Vec<u8>), String> {
    let mut acc = Vec::with_capacity(2048);
    let mut temp = [0u8; 1024];
    loop {
        let n = stream.read(&mut temp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Connection closed before HTTP headers completed".to_string());
        }
        acc.extend_from_slice(&temp[..n]);
        if let Some(pos) = find_double_crlf(&acc) {
            let header_bytes = &acc[..pos];
            let body_remainder = acc[(pos + 4)..].to_vec();
            let header_str = String::from_utf8_lossy(header_bytes);
            let status_code = header_str
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0);
            return Ok((status_code, body_remainder));
        }
        if acc.len() > 32768 {
            return Err("HTTP header exceeded 32 KB limit".to_string());
        }
    }
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}
