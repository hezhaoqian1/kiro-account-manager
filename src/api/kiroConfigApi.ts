// KiroConfig（agents / hooks / powers / skills / steering）API 调用
import { invoke } from '@tauri-apps/api/core'

// ============================================================
// 自定义 Agents
// ============================================================

export function getCustomAgents<T = any[]>(projectDir: string | null = null) {
  return invoke<T>('get_custom_agents', { projectDir })
}

export function saveCustomAgent(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke('save_custom_agent', { fileName, content, scope, projectDir })
}

export function deleteCustomAgent(fileName: string, scope: string, projectDir: string | null = null) {
  return invoke('delete_custom_agent', { fileName, scope, projectDir })
}

export function createCustomAgent<T = any>(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke<T>('create_custom_agent', { fileName, content, scope, projectDir })
}

// ============================================================
// Hooks
// ============================================================

export function getHooks<T = any[]>(projectDir: string | null = null) {
  return invoke<T>('get_hooks', { projectDir })
}

// scope: "user" = 用户级（~/.kiro/hooks，Kiro IDE 1.0.182+）；"project" = 项目级
export function saveHook(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke('save_hook', { fileName, content, scope, projectDir })
}

export function deleteHook(fileName: string, scope: string, projectDir: string | null = null) {
  return invoke('delete_hook', { fileName, scope, projectDir })
}

export function createHook<T = any>(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke<T>('create_hook', { fileName, content, scope, projectDir })
}

// ============================================================
// Powers
// ============================================================

export function getPowers<T = any[]>() {
  return invoke<T>('get_powers')
}

export function getPowerRegistries<T = any[]>() {
  return invoke<T>('get_power_registries')
}

export function getRecommendedPowers<T = any[]>() {
  return invoke<T>('get_recommended_powers')
}

export function installPower(name: string, cloneUrl: string, pathInRepo: string, branch: string) {
  return invoke('install_power', { name, cloneUrl, pathInRepo, branch })
}

export function uninstallPower(name: string) {
  return invoke('uninstall_power', { name })
}

// ============================================================
// Skills / Steering
// ============================================================

export function getSkills<T = any[]>(projectDir: string | null = null) {
  return invoke<T>('get_skills', { projectDir })
}

export function saveSkill(name: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke('save_skill', { name, content, scope, projectDir })
}

export function deleteSkill(name: string, scope: string, projectDir: string | null = null) {
  return invoke('delete_skill', { name, scope, projectDir })
}

export function createSkill<T = any>(name: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke<T>('create_skill', { name, content, scope, projectDir })
}

export function getSteeringFiles<T = any[]>(projectDir: string | null = null) {
  return invoke<T>('get_steering_files', { projectDir })
}

export function saveSteeringFile(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke('save_steering_file', { fileName, content, scope, projectDir })
}

export function deleteSteeringFile(fileName: string, scope: string, projectDir: string | null = null) {
  return invoke('delete_steering_file', { fileName, scope, projectDir })
}

export function createSteeringFile<T = any>(fileName: string, content: string, scope: string, projectDir: string | null = null) {
  return invoke<T>('create_steering_file', { fileName, content, scope, projectDir })
}

export function createDefaultSteeringFile<T = any>(scope: string, projectDir: string | null = null) {
  return invoke<T>('create_default_steering_file', { scope, projectDir })
}

export function createInitialProjectSteering<T = any>(projectDir: string) {
  return invoke<T>('create_initial_project_steering', { projectDir })
}

export function refineSteeringFile<T = any>(fileName: string, scope: string, projectDir: string | null = null) {
  return invoke<T>('refine_steering_file', { fileName, scope, projectDir })
}

// ============================================================
// MCP
// ============================================================

export function saveMcpServer(name: string, config: any, projectDir: string | null = null) {
  return invoke('save_mcp_server', { name, config, projectDir })
}

// ============================================================
// Skill Import
// ============================================================

export function importSkillLocal<T = any>(sourcePath: string, scope: string, projectDir: string | null = null, overwrite = false) {
  return invoke<T>('import_skill_local', { sourcePath, scope, projectDir, overwrite })
}

export function importSkillFromGithub<T = any>(args: {
  repoUrl: string
  pathInRepo: string | null
  branch: string | null
  targetName: string | null
  scope: string
  projectDir: string | null
  overwrite?: boolean
}) {
  return invoke<T>('import_skill_from_github', args)
}
