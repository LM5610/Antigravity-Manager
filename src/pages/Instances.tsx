import React, { useEffect, useState, useMemo } from 'react';
import {
  Layers,
  Play,
  Square,
  ExternalLink,
  FolderOpen,
  Plus,
  RefreshCw,
  Trash2,
  Settings2,
  Copy,
  Check,
  User,
  Terminal,
  FolderTree,
  AlertCircle,
  Sparkles,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useTranslation } from 'react-i18next';
import { showToast } from '../components/common/ToastContainer';
import ModalDialog from '../components/common/ModalDialog';
import { useAccountStore } from '../stores/useAccountStore';
import type {
  CreateInstanceParams,
  InstanceDefaults,
  InstanceProfileView,
  UpdateInstanceParams,
} from '../types/instance';
import * as instanceService from '../services/instanceService';
import { cn } from '../utils/cn';

export default function Instances() {
  const { t } = useTranslation();
  const { accounts, fetchAccounts } = useAccountStore();

  const [instances, setInstances] = useState<InstanceProfileView[]>([]);
  const [defaults, setDefaults] = useState<InstanceDefaults | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [actionLoadingId, setActionLoadingId] = useState<string | null>(null);

  // 对话框状态
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [editModalInstance, setEditModalInstance] = useState<InstanceProfileView | null>(null);
  const [deleteModalInstance, setDeleteModalInstance] = useState<InstanceProfileView | null>(null);
  const [deleteRemoveFiles, setDeleteRemoveFiles] = useState(false);

  // 复制反馈
  const [copiedId, setCopiedId] = useState<string | null>(null);

  // 表单状态 - 新建
  const [createForm, setCreateForm] = useState<CreateInstanceParams>({
    name: '',
    userDataDir: '',
    workingDir: '',
    extraArgs: '',
    bindAccountId: '',
    initMode: 'copy',
  });

  // 加载数据
  const loadData = async (silent = false) => {
    if (!silent) setLoading(true);
    else setRefreshing(true);
    try {
      const [list, defs] = await Promise.all([
        instanceService.listInstances(),
        instanceService.getInstanceDefaults(),
      ]);
      setInstances(list);
      setDefaults(defs);
    } catch (err: any) {
      console.error('加载实例失败:', err);
      showToast(t('instances.load_failed', '加载实例失败: ') + String(err), 'error');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    loadData();
    fetchAccounts();

    // 5秒自动定时轮询刷新运行状态（PID）
    const timer = setInterval(() => {
      loadData(true);
    }, 5000);
    return () => clearInterval(timer);
  }, []);

  // 准备新建实例表单
  const handleOpenCreate = () => {
    const nextIndex = instances.filter((i) => !i.isDefault).length + 1;
    const initialName = `Workspace-${nextIndex}`;
    const initialDir = defaults?.rootDir
      ? `${defaults.rootDir.replace(/\\/g, '/')}/${initialName}`
      : '';

    setCreateForm({
      name: initialName,
      userDataDir: initialDir,
      workingDir: '',
      extraArgs: '',
      bindAccountId: '',
      initMode: 'copy',
    });
    setCreateModalOpen(true);
  };

  // 名称变动时动态更新路径
  const handleNameChange = (name: string) => {
    const safeName = name.trim().replace(/[\\/:*?"<>|]/g, '_');
    const newDir = defaults?.rootDir
      ? `${defaults.rootDir.replace(/\\/g, '/')}/${safeName}`
      : createForm.userDataDir;
    setCreateForm((prev) => ({
      ...prev,
      name,
      userDataDir: newDir,
    }));
  };

  // 提交新建
  const handleCreateSubmit = async () => {
    if (!createForm.name.trim()) {
      showToast(t('instances.name_required', '请输入实例名称'), 'warning');
      return;
    }
    if (!createForm.userDataDir.trim()) {
      showToast(t('instances.dir_required', '请输入或选择实例数据目录'), 'warning');
      return;
    }

    try {
      setActionLoadingId('create');
      await instanceService.createInstance(createForm);
      showToast(t('instances.create_success', '实例创建成功'), 'success');
      setCreateModalOpen(false);
      await loadData(true);
    } catch (err: any) {
      showToast(t('instances.create_error', '创建实例失败: ') + String(err), 'error');
    } finally {
      setActionLoadingId(null);
    }
  };

  // 保存编辑
  const handleSaveEdit = async () => {
    if (!editModalInstance) return;
    try {
      setActionLoadingId(`edit-${editModalInstance.id}`);
      await instanceService.updateInstance({
        instanceId: editModalInstance.id,
        name: editModalInstance.name,
        workingDir: editModalInstance.workingDir || '',
        extraArgs: editModalInstance.extraArgs,
        bindAccountId: editModalInstance.bindAccountId,
      });
      showToast(t('instances.update_success', '配置更新成功'), 'success');
      setEditModalInstance(null);
      await loadData(true);
    } catch (err: any) {
      showToast(t('instances.update_error', '更新失败: ') + String(err), 'error');
    } finally {
      setActionLoadingId(null);
    }
  };

  // 删除实例确认
  const handleConfirmDelete = async () => {
    if (!deleteModalInstance) return;
    try {
      setActionLoadingId(`delete-${deleteModalInstance.id}`);
      await instanceService.deleteInstance(deleteModalInstance.id, deleteRemoveFiles);
      showToast(t('instances.delete_success', '实例已成功删除'), 'success');
      setDeleteModalInstance(null);
      await loadData(true);
    } catch (err: any) {
      showToast(t('instances.delete_error', '删除实例失败: ') + String(err), 'error');
    } finally {
      setActionLoadingId(null);
    }
  };

  // 启动实例
  const handleStart = async (instance: InstanceProfileView) => {
    try {
      setActionLoadingId(instance.id);
      await instanceService.startInstance(instance.id);
      showToast(t('instances.start_success', '已触发启动指令，正在启动 Antigravity IDE...'), 'success');
      await loadData(true);
    } catch (err: any) {
      showToast(t('instances.start_error', '启动失败: ') + String(err), 'error');
    } finally {
      setActionLoadingId(null);
    }
  };

  // 停止实例
  const handleStop = async (instance: InstanceProfileView) => {
    try {
      setActionLoadingId(instance.id);
      await instanceService.stopInstance(instance.id);
      showToast(t('instances.stop_success', '实例已成功关闭'), 'success');
      await loadData(true);
    } catch (err: any) {
      showToast(t('instances.stop_error', '关闭失败: ') + String(err), 'error');
    } finally {
      setActionLoadingId(null);
    }
  };

  // 聚焦窗口
  const handleFocus = async (instance: InstanceProfileView) => {
    if (!instance.lastPid) return;
    try {
      await instanceService.focusInstanceWindow(instance.lastPid);
    } catch (err: any) {
      showToast(t('instances.focus_error', '窗口唤起失败: ') + String(err), 'warning');
    }
  };

  // 打开文件夹
  const handleOpenFolder = async (dir: string) => {
    try {
      await instanceService.openInstanceFolder(dir);
    } catch (err: any) {
      showToast(t('instances.open_folder_error', '打开文件夹失败: ') + String(err), 'warning');
    }
  };

  // 复制路径
  const handleCopyPath = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  };

  // 账号查找辅助
  const accountMap = useMemo(() => {
    const map = new Map<string, string>();
    accounts.forEach((a) => {
      map.set(a.id, a.email);
    });
    return map;
  }, [accounts]);

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6 space-y-6">
      {/* 头部导航区域 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-base-200/60 pb-5">
        <div className="space-y-1">
          <div className="flex items-center gap-3">
            <div className="p-2.5 rounded-xl bg-blue-500/10 text-blue-600 dark:text-blue-400">
              <Layers className="w-6 h-6" />
            </div>
            <div>
              <h1 className="text-2xl font-bold tracking-tight text-base-content flex items-center gap-2">
                {t('instances.title', 'Antigravity IDE 应用多开')}
                <span className="badge badge-primary badge-outline text-xs font-medium">Multi-Instance</span>
              </h1>
              <p className="text-sm text-base-content/60">
                {t(
                  'instances.subtitle',
                  '支持多开多个独立的 Antigravity IDE 实例，各实例数据目录、已装扩展与登录凭据完全隔离'
                )}
              </p>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={() => loadData(true)}
            disabled={refreshing}
            className="btn btn-ghost btn-sm gap-2 border border-base-300 hover:bg-base-200"
            title={t('common.refresh', '刷新')}
          >
            <RefreshCw className={cn('w-4 h-4', refreshing && 'animate-spin text-primary')} />
            <span className="text-xs">{t('common.refresh', '刷新')}</span>
          </button>

          <button
            onClick={handleOpenCreate}
            className="btn btn-primary btn-sm gap-2 shadow-sm shadow-primary/20"
          >
            <Plus className="w-4 h-4" />
            <span>{t('instances.create_btn', '新建实例')}</span>
          </button>
        </div>
      </div>

      {/* 状态统计卡片 */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div className="p-4 rounded-2xl bg-base-100 border border-base-200/80 shadow-sm flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-primary/10 flex items-center justify-center text-primary font-bold text-lg">
            {instances.length}
          </div>
          <div>
            <div className="text-xs text-base-content/60 font-medium">
              {t('instances.stat_total', '实例总数')}
            </div>
            <div className="text-lg font-bold text-base-content">
              {instances.length} {t('instances.unit_instances', '个')}
            </div>
          </div>
        </div>

        <div className="p-4 rounded-2xl bg-base-100 border border-base-200/80 shadow-sm flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-emerald-500/10 flex items-center justify-center text-emerald-500 font-bold text-lg">
            {instances.filter((i) => i.running).length}
          </div>
          <div>
            <div className="text-xs text-base-content/60 font-medium">
              {t('instances.stat_running', '正在运行')}
            </div>
            <div className="text-lg font-bold text-emerald-600 dark:text-emerald-400">
              {instances.filter((i) => i.running).length} {t('instances.unit_instances', '个')}
            </div>
          </div>
        </div>

        <div className="p-4 rounded-2xl bg-base-100 border border-base-200/80 shadow-sm flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-indigo-500/10 flex items-center justify-center text-indigo-500 font-bold text-lg">
            <Sparkles className="w-6 h-6" />
          </div>
          <div>
            <div className="text-xs text-base-content/60 font-medium">
              {t('instances.stat_bound', '已绑定独立账号')}
            </div>
            <div className="text-lg font-bold text-base-content">
              {instances.filter((i) => i.bindAccountId).length} {t('instances.unit_instances', '个')}
            </div>
          </div>
        </div>
      </div>

      {/* 实例列表 Grid */}
      {loading ? (
        <div className="h-64 flex flex-col items-center justify-center gap-3 text-base-content/60">
          <span className="loading loading-spinner loading-lg text-primary"></span>
          <span className="text-sm">{t('common.loading', '正在探测并加载实例...')}</span>
        </div>
      ) : instances.length === 0 ? (
        <div className="h-72 border-2 border-dashed border-base-300 rounded-3xl flex flex-col items-center justify-center gap-4 p-8 text-center bg-base-100/50">
          <div className="p-4 rounded-2xl bg-base-200/60 text-base-content/40">
            <Layers className="w-10 h-10" />
          </div>
          <div className="space-y-1">
            <div className="text-base font-semibold text-base-content">
              {t('instances.empty_title', '暂无多开实例')}
            </div>
            <div className="text-xs text-base-content/50 max-w-sm">
              {t('instances.empty_desc', '点击右上角“新建实例”，即可一键克隆环境或创建全新沙箱目录。')}
            </div>
          </div>
          <button onClick={handleOpenCreate} className="btn btn-primary btn-sm gap-2 mt-2">
            <Plus className="w-4 h-4" />
            {t('instances.create_btn', '新建实例')}
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <AnimatePresence>
            {instances.map((instance) => {
              const isActionBusy = actionLoadingId === instance.id;
              const boundEmail = instance.bindAccountId ? accountMap.get(instance.bindAccountId) : null;

              return (
                <motion.div
                  key={instance.id}
                  layout
                  initial={{ opacity: 0, y: 15 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.95 }}
                  transition={{ duration: 0.2 }}
                  className={cn(
                    'group relative rounded-2xl border bg-base-100 p-5 shadow-sm transition-all duration-200 flex flex-col justify-between gap-4',
                    instance.running
                      ? 'border-emerald-500/40 shadow-emerald-500/5 hover:border-emerald-500/60'
                      : 'border-base-200/80 hover:border-base-300'
                  )}
                >
                  {/* 卡片头部 */}
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex items-center gap-3">
                      <div
                        className={cn(
                          'p-3 rounded-xl flex items-center justify-center transition-colors',
                          instance.running
                            ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                            : 'bg-base-200 text-base-content/60'
                        )}
                      >
                        <Layers className="w-5 h-5" />
                      </div>

                      <div>
                        <div className="flex items-center gap-2 flex-wrap">
                          <h3 className="font-bold text-base text-base-content tracking-tight">
                            {instance.name}
                          </h3>
                          {instance.isDefault && (
                            <span className="badge badge-sm badge-neutral font-medium">
                              {t('instances.badge_default', '系统默认')}
                            </span>
                          )}
                          {instance.running ? (
                            <span className="badge badge-sm badge-success gap-1.5 py-2.5 font-medium shadow-sm">
                              <span className="w-1.5 h-1.5 rounded-full bg-emerald-100 animate-pulse"></span>
                              Running {instance.lastPid ? `(PID: ${instance.lastPid})` : ''}
                            </span>
                          ) : (
                            <span className="badge badge-sm badge-ghost text-base-content/50 gap-1.5">
                              <span className="w-1.5 h-1.5 rounded-full bg-base-content/30"></span>
                              Stopped
                            </span>
                          )}
                        </div>

                        {/* 绑定账号标签 */}
                        <div className="flex items-center gap-1.5 mt-1 text-xs text-base-content/60">
                          <User className="w-3.5 h-3.5 text-base-content/40" />
                          <span>{t('instances.bound_account', '绑定账号')}:</span>
                          {boundEmail ? (
                            <span className="font-medium text-primary bg-primary/10 px-1.5 py-0.5 rounded">
                              {boundEmail}
                            </span>
                          ) : instance.bindAccountId ? (
                            <span className="font-mono text-base-content/70">
                              {instance.bindAccountId}
                            </span>
                          ) : (
                            <span className="italic text-base-content/40">
                              {t('instances.no_bind', '跟随当前系统默认')}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>

                    {/* 右上角快捷按钮 */}
                    <div className="flex items-center gap-1 opacity-80 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={() => setEditModalInstance(instance)}
                        className="btn btn-ghost btn-xs btn-square hover:bg-base-200"
                        title={t('common.edit', '编辑设置')}
                      >
                        <Settings2 className="w-3.5 h-3.5 text-base-content/60" />
                      </button>

                      {!instance.isDefault && (
                        <button
                          onClick={() => {
                            setDeleteModalInstance(instance);
                            setDeleteRemoveFiles(false);
                          }}
                          className="btn btn-ghost btn-xs btn-square text-error/70 hover:text-error hover:bg-error/10"
                          title={t('common.delete', '删除实例')}
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  </div>

                  {/* 路径与详情信息 */}
                  <div className="space-y-2 text-xs">
                    {/* 数据目录 */}
                    <div className="bg-base-200/50 rounded-xl p-2.5 flex items-center justify-between gap-2 border border-base-200">
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        <FolderTree className="w-4 h-4 text-base-content/40 shrink-0" />
                        <span className="text-base-content/40 shrink-0 select-none">
                          {t('instances.data_dir', '数据目录')}:
                        </span>
                        <span
                          className="font-mono text-base-content/70 truncate select-all"
                          title={instance.userDataDir}
                        >
                          {instance.userDataDir}
                        </span>
                      </div>

                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          onClick={() => handleCopyPath(instance.id, instance.userDataDir)}
                          className="btn btn-ghost btn-xs btn-square"
                          title={t('common.copy', '复制完整路径')}
                        >
                          {copiedId === instance.id ? (
                            <Check className="w-3.5 h-3.5 text-success" />
                          ) : (
                            <Copy className="w-3.5 h-3.5 text-base-content/40" />
                          )}
                        </button>

                        <button
                          onClick={() => handleOpenFolder(instance.userDataDir)}
                          className="btn btn-ghost btn-xs btn-square"
                          title={t('instances.open_folder', '在文件管理器中打开')}
                        >
                          <FolderOpen className="w-3.5 h-3.5 text-base-content/40" />
                        </button>
                      </div>
                    </div>

                    {/* 启动参数（若有） */}
                    {instance.extraArgs && (
                      <div className="flex items-center gap-2 text-base-content/50 px-1 font-mono">
                        <Terminal className="w-3.5 h-3.5 shrink-0" />
                        <span className="truncate">{instance.extraArgs}</span>
                      </div>
                    )}
                  </div>

                  {/* 底部操作区 */}
                  <div className="pt-2 border-t border-base-200/60 flex items-center justify-between gap-3">
                    <div className="text-xs text-base-content/40">
                      {instance.lastLaunchedAt
                        ? `${t('instances.last_launch', '最近启动')}: ${new Date(
                            instance.lastLaunchedAt
                          ).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
                        : t('instances.never_launched', '尚未启动过')}
                    </div>

                    <div className="flex items-center gap-2">
                      {instance.running ? (
                        <>
                          {instance.lastPid && (
                            <button
                              onClick={() => handleFocus(instance)}
                              className="btn btn-ghost btn-sm gap-1.5 border border-base-300 hover:bg-base-200"
                              title={t('instances.focus_window', '将该实例窗口激活到前台')}
                            >
                              <ExternalLink className="w-3.5 h-3.5 text-primary" />
                              <span className="text-xs">{t('instances.focus', '激活窗口')}</span>
                            </button>
                          )}

                          <button
                            onClick={() => handleStop(instance)}
                            disabled={isActionBusy}
                            className="btn btn-error btn-outline btn-sm gap-1.5"
                          >
                            <Square className="w-3.5 h-3.5" />
                            <span className="text-xs">{t('instances.stop', '停止')}</span>
                          </button>
                        </>
                      ) : (
                        <button
                          onClick={() => handleStart(instance)}
                          disabled={isActionBusy}
                          className="btn btn-primary btn-sm gap-1.5 shadow-sm shadow-primary/20"
                        >
                          {isActionBusy ? (
                            <span className="loading loading-spinner loading-xs"></span>
                          ) : (
                            <Play className="w-3.5 h-3.5 fill-current" />
                          )}
                          <span className="text-xs">{t('instances.start', '启动实例')}</span>
                        </button>
                      )}
                    </div>
                  </div>
                </motion.div>
              );
            })}
          </AnimatePresence>
        </div>
      )}

      {/* 新建实例对话框 */}
      <ModalDialog
        isOpen={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        title={t('instances.modal_create_title', '创建 Antigravity 实例')}
        size="md"
      >
        <div className="space-y-4 py-2">
          {/* 实例名称 */}
          <div>
            <label className="label py-1">
              <span className="label-text font-medium text-xs">
                {t('instances.form_name', '实例名称')} <span className="text-error">*</span>
              </span>
            </label>
            <input
              type="text"
              value={createForm.name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="e.g. Workspace-Dev, Claude-Testing"
              className="input input-bordered input-sm w-full font-medium"
            />
          </div>

          {/* 初始化模式 */}
          <div>
            <label className="label py-1">
              <span className="label-text font-medium text-xs">
                {t('instances.form_init_mode', '初始化模式')}
              </span>
            </label>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => setCreateForm((prev) => ({ ...prev, initMode: 'copy' }))}
                className={cn(
                  'p-3 rounded-xl border text-left flex flex-col gap-1 transition-all',
                  createForm.initMode === 'copy'
                    ? 'border-primary bg-primary/5 text-primary'
                    : 'border-base-300 hover:bg-base-200/50 text-base-content'
                )}
              >
                <div className="font-semibold text-xs flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5" />
                  {t('instances.mode_copy', '克隆现有配置 (推荐)')}
                </div>
                <div className="text-[11px] opacity-70">
                  {t('instances.mode_copy_desc', '复制当前已安装的插件与设置，开箱即用')}
                </div>
              </button>

              <button
                type="button"
                onClick={() => setCreateForm((prev) => ({ ...prev, initMode: 'empty' }))}
                className={cn(
                  'p-3 rounded-xl border text-left flex flex-col gap-1 transition-all',
                  createForm.initMode === 'empty'
                    ? 'border-primary bg-primary/5 text-primary'
                    : 'border-base-300 hover:bg-base-200/50 text-base-content'
                )}
              >
                <div className="font-semibold text-xs flex items-center gap-1.5">
                  <FolderTree className="w-3.5 h-3.5" />
                  {t('instances.mode_empty', '全新纯净空白')}
                </div>
                <div className="text-[11px] opacity-70">
                  {t('instances.mode_empty_desc', '创建空白数据目录，零插件无污染沙箱')}
                </div>
              </button>
            </div>
          </div>

          {/* 数据目录 */}
          <div>
            <label className="label py-1">
              <span className="label-text font-medium text-xs">
                {t('instances.form_dir', '数据目录 (User Data Dir)')} <span className="text-error">*</span>
              </span>
            </label>
            <input
              type="text"
              value={createForm.userDataDir}
              onChange={(e) => setCreateForm((prev) => ({ ...prev, userDataDir: e.target.value }))}
              placeholder="e.g. C:/Users/.../instances/Workspace-1"
              className="input input-bordered input-sm w-full font-mono text-xs"
            />
            <span className="text-[11px] text-base-content/50 mt-1 block">
              {t(
                'instances.form_dir_hint',
                '每个实例独占此目录，Electron 单例锁与配置均在此目录隔离'
              )}
            </span>
          </div>

          {/* 绑定账号 */}
          <div>
            <label className="label py-1">
              <span className="label-text font-medium text-xs">
                {t('instances.form_bind_account', '自动登录绑定账号 (可选)')}
              </span>
            </label>
            <select
              value={createForm.bindAccountId || ''}
              onChange={(e) =>
                setCreateForm((prev) => ({ ...prev, bindAccountId: e.target.value || undefined }))
              }
              className="select select-bordered select-sm w-full text-xs"
            >
              <option value="">{t('instances.select_no_bind', '-- 不绑定 (由 IDE 内登录) --')}</option>
              {accounts.map((acc) => (
                <option key={acc.id} value={acc.id}>
                  {acc.email} ({acc.token.is_gcp_tos ? 'Enterprise' : 'Standard'})
                </option>
              ))}
            </select>
            <span className="text-[11px] text-base-content/50 mt-1 block">
              {t('instances.form_bind_hint', '启动实例前将直接把该账号凭据写入该实例数据库，免扫码自动登录')}
            </span>
          </div>

          {/* 启动参数 */}
          <div>
            <label className="label py-1">
              <span className="label-text font-medium text-xs">
                {t('instances.form_extra_args', '附加启动参数 (可选)')}
              </span>
            </label>
            <input
              type="text"
              value={createForm.extraArgs || ''}
              onChange={(e) => setCreateForm((prev) => ({ ...prev, extraArgs: e.target.value }))}
              placeholder="e.g. --disable-gpu --no-sandbox"
              className="input input-bordered input-sm w-full font-mono text-xs"
            />
          </div>

          {/* 对话框按钮 */}
          <div className="modal-action pt-4 border-t border-base-200">
            <button
              type="button"
              onClick={() => setCreateModalOpen(false)}
              className="btn btn-ghost btn-sm"
            >
              {t('common.cancel', '取消')}
            </button>
            <button
              type="button"
              onClick={handleCreateSubmit}
              disabled={actionLoadingId === 'create'}
              className="btn btn-primary btn-sm gap-2"
            >
              {actionLoadingId === 'create' && <span className="loading loading-spinner loading-xs"></span>}
              {t('common.confirm', '确认创建')}
            </button>
          </div>
        </div>
      </ModalDialog>

      {/* 编辑实例对话框 */}
      <ModalDialog
        isOpen={!!editModalInstance}
        onClose={() => setEditModalInstance(null)}
        title={t('instances.modal_edit_title', '编辑实例配置')}
        size="md"
      >
        {editModalInstance && (
          <div className="space-y-4 py-2">
            <div>
              <label className="label py-1">
                <span className="label-text font-medium text-xs">
                  {t('instances.form_name', '实例名称')}
                </span>
              </label>
              <input
                type="text"
                disabled={editModalInstance.isDefault}
                value={editModalInstance.name}
                onChange={(e) =>
                  setEditModalInstance((prev) => (prev ? { ...prev, name: e.target.value } : null))
                }
                className="input input-bordered input-sm w-full font-medium"
              />
            </div>

            <div>
              <label className="label py-1">
                <span className="label-text font-medium text-xs">
                  {t('instances.form_bind_account', '绑定账号')}
                </span>
              </label>
              <select
                value={editModalInstance.bindAccountId || ''}
                onChange={(e) =>
                  setEditModalInstance((prev) =>
                    prev ? { ...prev, bindAccountId: e.target.value || null } : null
                  )
                }
                className="select select-bordered select-sm w-full text-xs"
              >
                <option value="">{t('instances.select_no_bind', '-- 不绑定 --')}</option>
                {accounts.map((acc) => (
                  <option key={acc.id} value={acc.id}>
                    {acc.email}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="label py-1">
                <span className="label-text font-medium text-xs">
                  {t('instances.form_extra_args', '附加启动参数')}
                </span>
              </label>
              <input
                type="text"
                value={editModalInstance.extraArgs || ''}
                onChange={(e) =>
                  setEditModalInstance((prev) =>
                    prev ? { ...prev, extraArgs: e.target.value } : null
                  )
                }
                placeholder="--disable-gpu"
                className="input input-bordered input-sm w-full font-mono text-xs"
              />
            </div>

            <div className="modal-action pt-4 border-t border-base-200">
              <button
                type="button"
                onClick={() => setEditModalInstance(null)}
                className="btn btn-ghost btn-sm"
              >
                {t('common.cancel', '取消')}
              </button>
              <button
                type="button"
                onClick={handleSaveEdit}
                className="btn btn-primary btn-sm"
              >
                {t('common.save', '保存更改')}
              </button>
            </div>
          </div>
        )}
      </ModalDialog>

      {/* 删除确认对话框 */}
      <ModalDialog
        isOpen={!!deleteModalInstance}
        onClose={() => setDeleteModalInstance(null)}
        title={t('instances.modal_delete_title', '确认删除实例')}
        size="sm"
      >
        {deleteModalInstance && (
          <div className="space-y-4 py-2">
            <div className="flex items-start gap-3 text-warning">
              <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
              <div className="text-xs text-base-content/80 leading-relaxed">
                {t('instances.delete_confirm_desc', '确定要移除实例')} <strong>"{deleteModalInstance.name}"</strong> {t('instances.delete_confirm_tail', '吗？此操作将从列表中注销该多开实例。')}
              </div>
            </div>

            <label className="label cursor-pointer justify-start gap-2.5 p-2 rounded-xl bg-base-200/50 border border-base-200">
              <input
                type="checkbox"
                checked={deleteRemoveFiles}
                onChange={(e) => setDeleteRemoveFiles(e.target.checked)}
                className="checkbox checkbox-error checkbox-xs"
              />
              <span className="label-text text-xs text-error font-medium">
                {t('instances.delete_files_label', '同时永久删除该实例的数据文件夹 (不可撤销)')}
              </span>
            </label>

            <div className="modal-action pt-4 border-t border-base-200">
              <button
                type="button"
                onClick={() => setDeleteModalInstance(null)}
                className="btn btn-ghost btn-sm"
              >
                {t('common.cancel', '取消')}
              </button>
              <button
                type="button"
                onClick={handleConfirmDelete}
                className="btn btn-error btn-sm"
              >
                {t('common.delete', '确认删除')}
              </button>
            </div>
          </div>
        )}
      </ModalDialog>
    </div>
  );
}
