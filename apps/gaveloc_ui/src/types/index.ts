// Account types
export interface Account {
  id: string;
  username: string;
  is_steam: boolean;
  is_free_trial: boolean;
  use_otp: boolean;
  last_login: number | null;
}

export interface CreateAccountRequest {
  username: string;
  is_steam: boolean;
  is_free_trial: boolean;
  use_otp: boolean;
}

export type Language = 'Japanese' | 'English' | 'German' | 'French';

// News Types
export interface NewsItem {
    date: string;
    title: string;
    url: string;
    id: string;
    tag: string;
}

export interface Headlines {
    news: NewsItem[];
    topics: NewsItem[];
    pinned: NewsItem[];
}

export interface Banner {
    image_url: string;
    link_url: string;
}

export interface NewsArticle {
    title: string;
    content_html: string;
    date: string;
    url: string;
}

// Settings types (mirrors gaveloc_core::config::Settings)
export interface Settings {
  game: GameSettings;
  wine: WineSettings;
  log_level: string;
}

export interface GameSettings {
  path: string | null;
  language: Language;
  gamemode: boolean;
  mangohud: boolean;
  gamescope: boolean;
  gamescope_settings: GamescopeSettings;
}

export interface WineSettings {
  runner_path: string | null;
  prefix_path: string | null;
  esync: boolean;
  fsync: boolean;
  winesync: boolean;
  dxvk_hud: string | null;
}

export interface GamescopeSettings {
  width: number | null;
  height: number | null;
  refresh_rate: number | null;
  fullscreen: boolean;
  borderless: boolean;
  extra_args: string | null;
}

// Runner types
export interface WineRunner {
  path: string;
  name: string;
  runner_type: RunnerType;
  is_valid: boolean;
}

export type RunnerType = 'System' | 'Proton' | 'Lutris' | 'GavelocManaged' | 'Custom';

// Patching types
export interface PatchEntry {
  version: string;
  url: string;
  sizeBytes: number;
}

export type PatchPhase =
  | 'Idle'
  | 'Downloading'
  | 'Verifying'
  | 'Applying'
  | 'Completed'
  | 'Failed'
  | 'Cancelled';

export interface PatchProgress {
  state: PatchPhase;
  currentPatch?: string;
  currentIndex: number;
  totalPatches: number;
  bytesDownloaded: number;
  totalBytes: number;
  speedBytesPerSec: number;
}

export interface PatchProgressEvent {
  phase: PatchPhase;
  current_index: number;
  total_patches: number;
  version_id: string;
  repository: string;
  bytes_processed: number;
  bytes_total: number;
  speed_bytes_per_sec: number;
}

export interface PatchCompletedEvent {
  index: number;
  version_id: string;
  repository: string;
}

export interface PatchErrorEvent {
  message: string;
  recoverable: boolean;
}

export interface PatchStatus {
  is_patching: boolean;
  phase: PatchPhase;
  current_patch_index: number;
  total_patches: number;
  current_version_id: string | null;
  current_repository: string | null;
  bytes_downloaded: number;
  bytes_total: number;
  speed_bytes_per_sec: number;
}

// Authentication types
export interface LoginCredentials {
  username: string;
  password: string;
  otp?: string;
}

export interface LoginResult {
  success: boolean;
  session_id?: string;
  region?: number;
  max_expansion?: number;
  playable?: boolean;
  error?: string;
  error_type?: LoginErrorType;
}

export type LoginErrorType =
  | 'invalid_credentials'
  | 'invalid_otp'
  | 'account_locked'
  | 'maintenance'
  | 'rate_limited'
  | 'no_subscription'
  | 'terms_not_accepted'
  | 'unknown';

export interface CachedSession {
  valid: boolean;
  unique_id?: string;
  region?: number;
  max_expansion?: number;
  remaining_secs?: number;
}

export interface SessionStatus {
  has_session: boolean;
  is_valid: boolean;
  remaining_secs?: number;
}

export type LoginState =
  | 'LoggedOut'
  | 'LoggingIn'
  | 'AwaitingOtp'
  | 'LoggedIn'
  | 'Error';

// Integrity types
export interface IntegrityResult {
  total_files: number;
  valid_count: number;
  mismatch_count: number;
  missing_count: number;
  unreadable_count: number;
  problems: FileIntegrityResult[];
}

export interface FileIntegrityResult {
  relative_path: string;
  status: IntegrityStatus;
  expected_hash: string;
  actual_hash: string | null;
}

export type IntegrityStatus = 'Valid' | 'Mismatch' | 'Missing' | 'Unreadable';

export interface IntegrityProgress {
  current_file: string;
  files_checked: number;
  total_files: number;
  bytes_processed: number;
  total_bytes: number;
  percent: number;
}

export interface IntegrityStatusDto {
  is_checking: boolean;
  current_file: string | null;
  files_checked: number;
  total_files: number;
  bytes_processed: number;
  total_bytes: number;
}

export interface FileToRepair {
  relative_path: string;
  expected_hash: string;
}

export interface RepairResult {
  success_count: number;
  failure_count: number;
}

// Game version types
export interface GameVersions {
  boot: string | null;
  game: string | null;
  expansions: ExpansionVersion[];
  game_path_valid: boolean;
}

export interface ExpansionVersion {
  name: string;
  version: string | null;
  installed: boolean;
}

export interface PatchEntryDto {
  version_id: string;
  url: string;
  size_bytes: number;
  repository: string;
}

export interface UpdateCheckResult {
  has_updates: boolean;
  patches: PatchEntryDto[];
  total_size_bytes: number;
  error?: string;
}

// UI state types
export type View = 'launcher' | 'settings' | 'accounts';

export interface AppError {
  code: string;
  message: string;
  details?: string;
}
