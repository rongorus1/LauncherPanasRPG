export interface DeviceDiagnostics {
  total_memory_gb: number | null;
  processor_name: string | null;
  logical_cores: number | null;
  gpu_names: string[];
}

export async function deviceDiagnostics(): Promise<DeviceDiagnostics | null> {
  if (!("__TAURI_INTERNALS__" in window)) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DeviceDiagnostics>("get_device_diagnostics");
}
