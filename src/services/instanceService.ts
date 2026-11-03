import { request } from '../utils/request';
import type {
  CreateInstanceParams,
  InstanceDefaults,
  InstanceProfileView,
  UpdateInstanceParams,
} from '../types/instance';

export async function getInstanceDefaults(): Promise<InstanceDefaults> {
  return request<InstanceDefaults>('get_instance_defaults');
}

export async function listInstances(): Promise<InstanceProfileView[]> {
  return request<InstanceProfileView[]>('list_instances');
}

export async function createInstance(
  params: CreateInstanceParams
): Promise<InstanceProfileView> {
  return request<InstanceProfileView>('create_instance', {
    name: params.name,
    userDataDir: params.userDataDir,
    workingDir: params.workingDir,
    extraArgs: params.extraArgs,
    bindAccountId: params.bindAccountId,
    copySourceInstanceId: params.copySourceInstanceId,
    initMode: params.initMode,
  });
}

export async function updateInstance(
  params: UpdateInstanceParams
): Promise<InstanceProfileView> {
  return request<InstanceProfileView>('update_instance', {
    instanceId: params.instanceId,
    name: params.name,
    workingDir: params.workingDir,
    extraArgs: params.extraArgs,
    bindAccountId: params.bindAccountId,
  });
}

export async function deleteInstance(
  instanceId: string,
  removeFiles: boolean = false
): Promise<void> {
  return request<void>('delete_instance', {
    instanceId,
    removeFiles,
  });
}

export async function startInstance(
  instanceId: string
): Promise<InstanceProfileView> {
  return request<InstanceProfileView>('start_instance', {
    instanceId,
  });
}

export async function stopInstance(instanceId: string): Promise<void> {
  return request<void>('stop_instance', {
    instanceId,
  });
}

export async function focusInstanceWindow(pid: number): Promise<void> {
  return request<void>('focus_instance_window', {
    pid,
  });
}

export async function openInstanceFolder(userDataDir: string): Promise<void> {
  return request<void>('open_instance_folder', {
    userDataDir,
  });
}
