export interface InstanceProfileView {
  id: string;
  name: string;
  userDataDir: string;
  workingDir?: string | null;
  extraArgs: string;
  bindAccountId?: string | null;
  createdAt: number;
  lastLaunchedAt?: number | null;
  lastPid?: number | null;
  running: boolean;
  initialized: boolean;
  isDefault: boolean;
}

export interface InstanceDefaults {
  rootDir: string;
  defaultUserDataDir: string;
}

export interface CreateInstanceParams {
  name: string;
  userDataDir: string;
  workingDir?: string;
  extraArgs?: string;
  bindAccountId?: string;
  copySourceInstanceId?: string;
  initMode?: 'copy' | 'empty' | 'existing_dir';
}

export interface UpdateInstanceParams {
  instanceId: string;
  name?: string;
  workingDir?: string;
  extraArgs?: string;
  bindAccountId?: string | null;
}
