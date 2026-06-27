/** @jsxImportSource @opentui/solid */

import type { TuiPlugin, TuiPluginModule } from "@opencode-ai/plugin/tui"
import { readFileSync, existsSync } from "fs"
import { join } from "path"
import { homedir } from "os"
import { createSignal, For, Show } from "solid-js"

interface UsageEntry {
  command: string
  tokens_in: number
  tokens_out: number
  exit_code: number
  via: string
}

interface UsageStats {
  total_runs: number
  total_tokens_in: number
  total_tokens_out: number
  total_input_bytes: number
  total_output_bytes: number
  entries: UsageEntry[]
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

        const cost = usage.total_tokens_out * PAID_MODEL_COST_PER_TOKEN
        const saved = cost
        const recent = usage.entries.slice(-3).reverse()

        return (
          <box
            border
            borderColor="gray"
            flexDirection="column"
            gap={1}
            paddingTop={1}
            paddingBottom={1}
            paddingLeft={2}
            paddingRight={2}
          >
            <text>
              <b>Cotrex Usage</b>
            </text>
            <text> Runs: {formatNum(usage.total_runs)}</text>
            <text> Tokens: {formatNum(usage.total_tokens_out)} out</text>
            <text color="green"> Saved: {formatCost(saved)}</text>
            <text dim> vs paid model ($3/1M tokens)</text>
            <Show when={recent.length > 0}>
              <text dim> Recent:</text>
              <For each={recent}>
                {(e) => (
                  <text>
                    {"  "}{e.command.slice(0, 22)}{e.command.length > 22 ? ".." : ""} [{e.tokens_out}]
                  </text>
                )}
              </For>
            </Show>
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
