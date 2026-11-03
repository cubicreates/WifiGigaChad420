//! `giga_transport` — Bound Multi-Interface HTTP/HTTPS Range Downloader & Local System Proxy.

pub mod downloader;
pub mod proxy;

pub use downloader::{DownloadProgressSnapshot, MultiPathDownloader, ParsedUrl};
pub use proxy::{ProxyGatewayConfig, ProxyTelemetrySnapshot, SystemProxyGateway};
