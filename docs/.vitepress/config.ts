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
      { text: 'Commands', link: '/commands/init' },
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
            { text: 'Categories', link: '/guide/categories' }
          ]
        }
      ],
      '/commands/': [
        {
          text: 'Commands',
          items: [
            { text: 'init', link: '/commands/init' },
            { text: 'add', link: '/commands/add' },
            { text: 'remove', link: '/commands/remove' },
            { text: 'sync', link: '/commands/sync' },
            { text: 'pull', link: '/commands/pull' },
            { text: 'restore', link: '/commands/restore' },
            { text: 'status', link: '/commands/status' },
            { text: 'category', link: '/commands/category' },
            { text: 'info', link: '/commands/info' },
            { text: 'diff', link: '/commands/diff' }
          ]
        }
      ],
      '/advanced/': [
        {
          text: 'Advanced',
          items: [
            { text: 'Encryption', link: '/advanced/encryption' },
            { text: 'Multi-host Setup', link: '/advanced/multi-host' },
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
