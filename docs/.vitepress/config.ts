import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'confect',
  description: 'Manage your system configuration files with Git',

  base: '/confect/',

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/confect/logo.svg' }],
  ],

  themeConfig: {
    nav: [
      { text: 'Guide', link: '/guide/installation' },
      { text: 'Commands', link: '/commands/' },
      { text: 'GitHub', link: 'https://github.com/ursul/confect' }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Quick Start', link: '/guide/quick-start' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'Categories', link: '/guide/categories' },
            { text: 'Upgrading from 1.x', link: '/guide/migration' }
          ]
        }
      ],
      '/commands/': [
        {
          text: 'Commands',
          items: [
            { text: 'Overview', link: '/commands/' },
            { text: 'init', link: '/commands/init' },
            { text: 'add', link: '/commands/add' },
            { text: 'remove', link: '/commands/remove' },
            { text: 'status', link: '/commands/status' },
            { text: 'diff', link: '/commands/diff' },
            { text: 'sync', link: '/commands/sync' },
            { text: 'restore', link: '/commands/restore' },
            { text: 'pull', link: '/commands/pull' },
            { text: 'push', link: '/commands/push' },
            { text: 'category', link: '/commands/category' },
            { text: 'audit', link: '/commands/audit' },
            { text: 'key', link: '/commands/key' },
            { text: 'migrate', link: '/commands/migrate' },
            { text: 'info', link: '/commands/info' },
            { text: 'setup-timer', link: '/commands/setup-timer' },
            { text: 'self-update', link: '/commands/self-update' }
          ]
        }
      ],
      '/advanced/': [
        {
          text: 'Advanced',
          items: [
            { text: 'Secrets and Encryption', link: '/advanced/encryption' },
            { text: 'Multiple Hosts', link: '/advanced/multi-host' },
            { text: 'Systemd Timer', link: '/advanced/systemd' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/ursul/confect' }
    ],

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright (c) 2024 ursul'
    },

    search: {
      provider: 'local'
    }
  }
})
