---
title: Why apollo exists
date: 2026-09-14
tags: [philosophy]
---

Coding agents kept getting bigger: web dashboards, background daemons, workspace indexers, subscriptions. I wanted the opposite — the smallest tool that lets a strong model touch my terminal.

> Enter **apollo**.

## The philosophy

- **Terminal-native.** Your shell is the interface. No browser tab, no Electron, no context-switching.
- **Three tools.** `ls`, `read`, `bash` cover 95% of coding assistance. Fewer tools means fewer surprises.
- **Transparent.** Debug mode shows the exact request JSON. What leaves your machine is never a mystery.
- **One binary.** `go install`, set one env var, done.

## When not to use apollo

Honest answer: if you need multi-file autonomous refactoring with reviews and CI integration, use a heavier harness (or an IDE agent). Apollo is for fast, conversational help inside the terminal you already live in — and this page's [docs](../) cover everything it does.
