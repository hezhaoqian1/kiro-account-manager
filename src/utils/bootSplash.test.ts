import test from 'node:test'
import assert from 'node:assert/strict'
import { dismissBootSplash } from './bootSplash.js'

test('dismissBootSplash marks splash hidden and removes it', () => {
  let removed = false
  const splash = {
    dataset: {} as Record<string, string>,
    remove() {
      removed = true
    }}
  const documentMock = {
    getElementById(id: string) {
      return id === 'boot-splash' ? splash : null
    }}

  const result = dismissBootSplash(documentMock as unknown as Document)

  assert.equal(result, true)
  assert.equal(splash.dataset.state, 'hidden')
  assert.equal(removed, true)
})

test('dismissBootSplash returns false when splash is absent', () => {
  const documentMock = {
    getElementById() {
      return null
    }}

  const result = dismissBootSplash(documentMock as unknown as Document)

  assert.equal(result, false)
})