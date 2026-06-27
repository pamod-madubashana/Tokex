/** @jsxImportSource @opentui/solid */

import type { TuiPlugin, TuiPluginModule } from "@opencode-ai/plugin/tui"
import { readFileSync, existsSync } from "fs"
import { join } from "path"
import { homedir } from "os"
import { createSignal } from "solid-js"

interface UsageStats {
  total_runs: number
  total_tokens_out: number
}

const PAID_MODEL_COST_PER_TOKEN = 3.0 / 1_000_000.0
const COLLAPSED_KEY = "cotrex-usage-sidebar.collapsed"

function readUsage(): UsageStats | null {
  const paths = [
    join(homedir(), ".local", "share", "cotrex", "usage.json"),
    join(homedir(), ".config", "cotrex", "usage.json"),
    join(process.cwd(), ".cotrex", "usage.json"),
  ]
  for (const p of paths) {
    try {
      if (existsSync(p)) {
        return JSON.parse(readFileSync(p, "utf-8"))
      }
    } catch {}
  }
  return null
}

function formatNum(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}

function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  if (cost < 1.0) return `$${cost.toFixed(3)}`
  return `$${cost.toFixed(2)}`
}

const tui: TuiPlugin = async (api) => {
  const [collapsed, setCollapsed] = createSignal(Boolean(api.kv.get(COLLAPSED_KEY, false)))
  const [usageVersion, setUsageVersion] = createSignal(0)

  const toggleCollapsed = () => {
    const next = !collapsed()
    setCollapsed(next)
    api.kv.set(COLLAPSED_KEY, next)
  }

  api.slots.register({
    order: 150,
    slots: {
      sidebar_content: (_ctx, _props) => {
        usageVersion()

        const usage = readUsage()
        if (!usage || usage.total_runs === 0) return null

        const saved = usage.total_tokens_out * PAID_MODEL_COST_PER_TOKEN

        return (
          <box flexDirection="column">
            <text>Cotrex</text>
            <text dim>{formatNum(usage.total_runs)} runs</text>
            <text dim>{formatNum(usage.total_tokens_out)} tokens saved</text>
          </box>
        )
      },
    },
  })
}

const plugin: TuiPluginModule & { id: string } = {
  id: "cotrex-usage-sidebar",
  tui,
}

export default plugin
