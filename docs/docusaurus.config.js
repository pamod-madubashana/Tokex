module.exports = {
  title: 'Tokex',
  tagline: 'RTK executes. Tokex makes execution consumable by agents.',
  url: 'https://pamod-madubashana.github.io',
  baseUrl: '/Tokex/',
  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  favicon: 'img/tokex.png',
  organizationName: 'pamod-madubashana',
  projectName: 'Tokex',
  deploymentBranch: 'gh-pages',
  presets: [
    [
      '@docusaurus/preset-classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: require.resolve('./sidebars.js'),
          editUrl: 'https://github.com/pamod-madubashana/Tokex/edit/main/docs/',
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
      title: 'Tokex',
      logo: {
        alt: 'Tokex',
        src: 'img/tokex.png',
      },
      items: [
        { type: 'doc', docId: 'intro', position: 'left', label: 'Docs' },
        { type: 'doc', docId: 'mcp', position: 'left', label: 'MCP' },
        { type: 'doc', docId: 'downloads', position: 'left', label: 'Downloads' },
        {
          href: 'https://github.com/pamod-madubashana/Tokex',
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
            { label: 'Introduction', to: '/' },
            { label: 'Installation', to: '/installation' },
            { label: 'Setup', to: '/setup' },
            { label: 'MCP', to: '/mcp' },
          ],
        },
        {
          title: 'More',
          items: [
            { label: 'Downloads', to: '/downloads' },
            { label: 'GitHub', href: 'https://github.com/pamod-madubashana/Tokex' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Tokex.`,
    },
  },
};
