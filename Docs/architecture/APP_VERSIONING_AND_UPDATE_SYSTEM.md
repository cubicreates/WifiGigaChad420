# WiFi GigaChad 420 — Versioning and Update System Specification

## 1. Overview & Format Standards

WiFi GigaChad 420 implements an automated build versioning and mismatch-detection architecture designed to seamlessly notify users of updates, track installed binaries against running binaries, and surface the update UI whenever a version delta or mismatch is detected.

### Version String Pattern
The version numbering adheres to the strict naming convention:
- **Prefix**: `TST` (constant identifier)
- **Minor Release**: `0.1`, `0.2`, `0.3`, etc. (incremented on updates)
- **Suffix**: `3.11.2026` (constant build date / epoch stamp)

### Standard Progression
- Initial Version: `TST-0.1.3.11.2026`
- Target Update Version: `TST-0.2.3.11.2026`
- Format: `TST-<MINOR>.<PATCH>.<DAY>.<MONTH>.<YEAR>` where `TST` and `3.11.2026` are fixed invariants.

---

## 2. Architecture & File Storage

### Version Storage
The application stores the installed version string persistently on disk:
- **Primary Path**: `%LOCALAPPDATA%\WiFiGigaChad420\installed_version.txt`
- **Fallback Path**: `C:\Users\Public\WiFiGigaChad420\installed_version.txt`

### Mismatch Detection Logic
When the application binary executes:
1. `main.rs` initializes and reads `installed_version.txt` from disk via `read_installed_version_from_disk()`.
2. If the file exists and its content differs from the running binary's `CURRENT_APP_VERSION` (`TST-0.1.3.11.2026`):
   - A `version_mismatch` flag is set to `true`.
   - The detected previous version is captured into `previous_version = Some(...)`.
   - An alert warning is logged into the application logs:
     `"ATTENTION: App version mismatch detected! Previous installed: '...', Running binary: '...'."`
3. If no file exists (fresh install), the current binary version is written to disk.
4. When `POST /api/update-install` is invoked after updating, the stored version on disk is updated via `write_installed_version_to_disk()`.

---

## 3. UI Flow & Update Modal

### Automated Trigger
- When the frontend starts up, it queries `/api/update-check` and inspects `update_info`.
- If `update_info.version_mismatch` is `true` or `update_info.update_available` is `true`:
  - An automatic popup timer triggers after 1.8 seconds.
  - The UI displays the amber **"VERSION MISMATCH DETECTED"** banner.
  - It clearly contrasts the **Installed Version** against the **New Version**.
  - Provides a single-click **"Apply & Sync Version"** or **"Download & Install Update"** workflow.

### UI Components Updated
- [`UpdateModal.tsx`](file:///d:/Github/LightningMCQueen/Destop/Win/apps/desktop/ui/src/components/UpdateModal.tsx): Amber alert styling and badge comparison.
- [`Sidebar.tsx`](file:///d:/Github/LightningMCQueen/Destop/Win/apps/desktop/ui/src/components/Sidebar.tsx): Formatted version pill rendering.
- [`InstallerUninstallerScreens.tsx`](file:///d:/Github/LightningMCQueen/Destop/Win/apps/desktop/ui/src/screens/InstallerUninstallerScreens.tsx): Version badge synchronization.
- [`App.tsx`](file:///d:/Github/LightningMCQueen/Destop/Win/apps/desktop/ui/src/App.tsx): Startup polling and mismatch notification triggers.
- [`main.rs`](file:///d:/Github/LightningMCQueen/Destop/Win/apps/desktop/src/main.rs): Backend endpoints, constants, persistence, and unit tests.

