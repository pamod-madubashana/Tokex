import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import styles from './index.module.css';

const features = [
  {
    title: 'Dual-channel output',
    body: 'Structured NDJSON on stdout for the model; a readable summary on stderr for you. Small, typed events instead of noisy logs.',
  },
  {
    title: 'Three front-ends, one core',
    body: 'CLI, stdin-JSON, and a native MCP server all funnel into the same deterministic execution pipeline.',
  },
  {
    title: 'Standalone',
    body: 'Ship just tokex — it downloads its rtk backend for your OS, pinned to a tested version. Nothing else to install.',
  },
  {
    title: 'No lock-in',
    body: 'Tokex never bypasses RTK and never owns execution. Predictable routing, structured events, agent-agnostic.',
  },
];

const SNIPPET = `$ tokex run "cargo test"
{"type":"stdout","line":"test result: ok. 21 passed","severity":"info"}
{"type":"result","status":"ok","code":0}`;

export default function Home() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout title="Tokex" description={siteConfig.tagline}>
      <header className={styles.hero}>
        <img src={useBaseUrl('/img/tokex.png')} alt="Tokex" className={styles.logo} />
        <h1 className={styles.title}>Tokex</h1>
        <p className={styles.tagline}>
          RTK executes. <span className={styles.accent}>Tokex makes execution consumable by agents.</span>
        </p>
        <div className={styles.buttons}>
          <Link className={styles.primary} to="/docs">
            Get started
          </Link>
          <Link className={styles.secondary} href="https://github.com/pamod-madubashana/Tokex">
            GitHub
          </Link>
        </div>
        <pre className={styles.code}>{SNIPPET}</pre>
      </header>
      <main className={styles.features}>
        {features.map((f) => (
          <div key={f.title} className={styles.card}>
            <h3>{f.title}</h3>
            <p>{f.body}</p>
          </div>
        ))}
      </main>
    </Layout>
  );
}
