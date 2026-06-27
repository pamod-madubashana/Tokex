module.exports = {
  title: 'Cotrex',
  tagline: 'RTK executes. Cotrex makes execution consumable by agents.',
  url: 'https://pamod-madubashana.github.io',
  baseUrl: '/Cotrex/',
  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  favicon: 'img/cotrex.png',
  organizationName: 'pamod-madubashana',
  projectName: 'Cotrex',
  deploymentBranch: 'gh-pages',
  presets: [
    [
      '@docusaurus/preset-classic',
      {
        docs: {
          routeBasePath: '/docs',
          sidebarPath: require.resolve('./sidebars.js'),
          editUrl: 'https://github.com/pamod-madubashana/Cotrex/edit/main/docs/',
        },
        theme: {
          customCss: require.resolve('./src/css/custom.css'),
        },
      },
    ],
  ],
  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Cotrex',
      logo: {
        alt: 'Cotrex',
        src: 'img/cotrex.png',
      },
      items: [
        { type: 'doc', docId: 'intro', position: 'left', label: 'Docs' },
        { type: 'doc', docId: 'mcp', position: 'left', label: 'MCP' },
        { type: 'doc', docId: 'downloads', position: 'left', label: 'Downloads' },
        {
          href: 'https://github.com/pamod-madubashana/Cotrex',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            { label: 'Introduction', to: '/docs' },
            { label: 'Installation', to: '/docs/installation' },
            { label: 'Setup', to: '/docs/setup' },
            { label: 'MCP', to: '/docs/mcp' },
          ],
        },
        {
          title: 'More',
          items: [
            { label: 'Downloads', to: '/docs/downloads' },
            { label: 'GitHub', href: 'https://github.com/pamod-madubashana/Cotrex' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Cotrex.`,
    },
  },
};
