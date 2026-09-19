---
layout: home

hero:
  name: confect
  text: Config files, managed
  tagline: Keep a server's configuration files in Git, one branch per host, and put them back when you need them
  actions:
    - theme: brand
      text: Get Started
      link: /guide/installation
    - theme: alt
      text: Upgrading from 1.x
      link: /guide/migration
    - theme: alt
      text: View on GitHub
      link: https://github.com/ursul/confect

features:
  - icon: 📦
    title: Git-backed
    details: Every sync is a commit on the host's own branch, pushed to your remote with your system git and SSH setup
  - icon: 🔍
    title: Status, diff, restore
    details: See what changed on the system as a real unified diff, record it, or write the stored files back atomically
  - icon: 🔐
    title: Secrets stay secret
    details: Files matching encrypt patterns are stored with age; plaintext private keys and password hashes block the sync
  - icon: 📁
    title: Categories
    details: Group paths by purpose — nginx, ssh, cron — with exclusions and name patterns such as *.pem
  - icon: 🔒
    title: Permissions and owners
    details: Mode, owner and group of every file, directory and symlink are recorded and restored
  - icon: 🛟
    title: Disaster recovery
    details: On a rebuilt machine, init --from clones the host's branch and restore brings the configuration back
---
