import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import {
  buildClientSamples,
  buildGatewayBaseUrl,
  buildGatewayConnectHost,
  buildGatewayIntegrationSummary,
  buildGatewaySecuritySummary,
  countEffectiveClientApiKeys,
  createGatewayFieldErrors,
  formatGatewayAccountOptionLabel,
  getEffectiveClientApiKey,
  parseClientApiKeys} from './gatewayPageUtils.js'

test('gateway page does not expose the standalone account health dialog', async () => {
  const source = await readFile(new URL('./index.tsx', import.meta.url), 'utf8')

  assert.equal(source.includes('AccountHealthDialog'), false)
  assert.equal(source.includes('showAccountHealth'), false)
  assert.equal(source.includes('账号健康'), false)
})

test('formatGatewayAccountOptionLabel shows email with quota and status', () => {
  const label = formatGatewayAccountOptionLabel({
    email: 'foo@example.com',
    userId: 'user-id-foo',
    id: 'abc',
    status: 'active',
    usageData: { usageBreakdownList: [{ usageLimit: 100, currentUsage: 30 }] }
  })
  assert.equal(label, 'foo@example.com 剩余 70/100')
})

test('formatGatewayAccountOptionLabel shows banned status', () => {
  const label = formatGatewayAccountOptionLabel({
    email: 'test@example.com',
    userId: 'user-id',
    id: 'def',
    status: 'banned',
    usageData: { usageBreakdownList: [{ usageLimit: 100, currentUsage: 95 }] }
  })
  // banned 账号是 unavailable 状态，getQuota/getUsed 返回 0
  assert.strictEqual(label, 'test@example.com 剩余 0/0 [banned]')
})

test('formatGatewayAccountOptionLabel falls back to userId when email is missing', () => {
  assert.equal(
    formatGatewayAccountOptionLabel({
      userId: 'builder-user-1',
      id: '0d24370c-1111-2222-3333-444455556666',
      usageData: { usageBreakdownList: [{ usageLimit: 50, currentUsage: 10 }] }
    }),
    'builder-user-1 剩余 40/50'
  )

  assert.equal(
    formatGatewayAccountOptionLabel({
      id: '0d24370c-1111-2222-3333-444455556666',
      usageData: { usageBreakdownList: [{ usageLimit: 0, currentUsage: 0 }] }
    }),
    '未知账号 剩余 0/0'
  )
})

test('createGatewayFieldErrors rejects unsupported host values', () => {
  const errors = createGatewayFieldErrors({
    host: 'bad host',
    port: 8765,
    apiKey: 'sk-test',
    region: 'us-east-1',
    accountMode: 'single',
    accountId: 'account-1',
    groupId: null,
    allowedIpsText: ''})

  assert.equal(errors.host, '监听地址必须是 localhost、IPv4 或 IPv6 地址')
})

test('createGatewayFieldErrors requires allowlist when remote access is enabled', () => {
  const errors = createGatewayFieldErrors({
    host: '0.0.0.0',
    port: 8765,
    apiKey: 'sk-test',
    region: 'us-east-1',
    accountMode: 'single',
    accountId: 'account-1',
    groupId: null,
    localOnly: false,
    allowedIpsText: ''})

  assert.equal(errors.allowedIpsText, '允许远程访问时必须至少配置一个白名单来源 IP')
})

test('buildGatewayConnectHost maps wildcard binds to a usable client host', () => {
  assert.equal(buildGatewayConnectHost('0.0.0.0', true), '127.0.0.1')
  assert.equal(buildGatewayConnectHost('0.0.0.0', false), 'localhost')
  assert.equal(buildGatewayConnectHost('::', false), 'localhost')
})

test('buildGatewayBaseUrl brackets ipv6 addresses for clients', () => {
  assert.equal(buildGatewayBaseUrl('::1', 8765, true), 'http://[::1]:8765')
  assert.equal(buildGatewayBaseUrl('0.0.0.0', 8765, false), 'http://localhost:8765')
})

test('buildGatewayBaseUrl uses the public web origin for headless remote access', () => {
  assert.equal(
    buildGatewayBaseUrl('0.0.0.0', 8080, false, 'https://kiro.example.com/'),
    'https://kiro.example.com',
  )
})

test('createGatewayFieldErrors accepts IPv6 wildcard allowlist entries', () => {
  const errors = createGatewayFieldErrors({
    host: '0.0.0.0',
    port: 8080,
    apiKey: 'sk-test',
    region: 'us-east-1',
    accountMode: 'pool',
    localOnly: false,
    allowedIpsText: '0.0.0.0/0\n::/0',
  })

  assert.equal(errors.allowedIpsText, undefined)
})

test('buildClientSamples redacts full api key in integration snippets', () => {
  const samples = buildClientSamples('http://127.0.0.1:8765', 'sk-super-secret-value')

  assert.ok(samples.anthropic.env.includes('ANTHROPIC_API_KEY=sk-'))
  assert.ok(samples.openai.env.includes('OPENAI_API_KEY=sk-'))
  assert.ok(samples.openai.curl.includes('Authorization: Bearer sk-'))
  assert.equal(samples.anthropic.env.includes('sk-super-secret-value'), false)
  assert.equal(samples.openai.env.includes('sk-super-secret-value'), false)
  assert.equal(samples.openai.curl.includes('sk-super-secret-value'), false)
})

test('buildGatewayIntegrationSummary does not expose full bearer token', () => {
  const summary = buildGatewayIntegrationSummary({
    baseUrl: 'http://127.0.0.1:8765',
    clientApiKeysText: 'sk-super-secret-value\nsk-secondary-value',
    logDir: 'C:/tmp/logs',
    errorHistory: []})

  assert.ok(summary.authLabel.startsWith('Bearer sk-'))
  assert.equal(summary.authLabel.includes('sk-super-secret-value'), false)
  assert.equal(summary.authLabel.includes('共 2 个 Key'), true)
})

test('parseClientApiKeys trims, deduplicates and drops blank lines', () => {
  assert.deepEqual(
    parseClientApiKeys(' sk-primary \n\nsk-secondary\nsk-primary \n , sk-third '),
    ['sk-primary', 'sk-secondary', 'sk-third']
  )
})

// 保真解析：hydrate 往返不能丢掉禁用标记，否则用户在 ApiKeysDialog
// 里关掉的 key 会被永久删除
test('parseClientApiKeys keeps disabled entries and name prefixes', () => {
  assert.deepEqual(
    parseClientApiKeys('#disabled#Key 1:sk-off\nsk-on'),
    ['#disabled#Key 1:sk-off', 'sk-on']
  )
})

// 与后端 effective_client_api_keys 对齐：跳过禁用项、剥离 name 前缀
test('getEffectiveClientApiKey skips disabled and strips name prefix', () => {
  assert.equal(getEffectiveClientApiKey('#disabled#Key 1:sk-off\nKey 2:sk-on'), 'sk-on')
  assert.equal(getEffectiveClientApiKey('sk-plain'), 'sk-plain')
  assert.equal(getEffectiveClientApiKey('#disabled#only:sk-x'), '')
})

test('countEffectiveClientApiKeys counts only enabled keys', () => {
  assert.equal(countEffectiveClientApiKeys('#disabled#a:sk-1\nb:sk-2'), 1)
  assert.equal(countEffectiveClientApiKeys(''), 0)
})

test('createGatewayFieldErrors requires at least one client api key', () => {
  const errors = createGatewayFieldErrors({
    host: '127.0.0.1',
    port: 8765,
    clientApiKeysText: ' \n ',
    region: 'us-east-1',
    accountMode: 'single',
    accountId: 'account-1',
    groupId: null,
    localOnly: true,
    allowedIpsText: ''})

  assert.equal(errors.clientApiKeysText, '必须至少填写一个客户端 API Key')
})

test('buildGatewaySecuritySummary reports multi-key state', () => {
  const summary = buildGatewaySecuritySummary({
    config: {
      clientApiKeysText: 'sk-primary\nsk-secondary',
      localOnly: false,
      allowedIpsText: '10.0.0.0/24',
      logLevel: 'info'}})

  assert.equal(summary.allowedIpsCount, 1)
  assert.equal(summary.apiKeyState, '已配置 2 个客户端 Key')
})
