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
        const runs = usage?.total_runs ?? 0
        const tokens = usage?.total_tokens_out ?? 0

        const theme = api.theme.current

        return (
          <box flexDirection="column">
            <text style={{ fg: theme.text }}>Cotrex</text>
            <text style={{ fg: theme.textMuted }}>{formatNum(runs)} runs</text>
            <text style={{ fg: theme.textMuted }}>{formatNum(tokens)} tokens saved</text>
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
