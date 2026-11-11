---
name: DesktopORZ GUI
description: Design system for DesktopORZ Windows icon layout manager interface
colors:
  primary: "#2563eb"
  primary-hover: "#1d4ed8"
  success: "#10b981"
  success-hover: "#059669"
  danger: "#ef4444"
  danger-hover: "#dc2626"
  surface-dark-bg: "#18181b"
  surface-dark-card: "#27272a"
  surface-light-bg: "#f4f4f5"
  surface-light-card: "#ffffff"
  text-primary-dark: "#f4f4f5"
  text-muted-dark: "#a1a1aa"
  text-primary-light: "#09090b"
  text-muted-light: "#71717a"
typography:
  display:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.2
  body:
    fontFamily: "Segoe UI, Inter, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: "Consolas, Cascadia Code, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.3
rounded:
  sm: "6px"
  md: "8px"
  lg: "10px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.text-primary-dark}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  button-success:
    backgroundColor: "{colors.success}"
    textColor: "{colors.text-primary-dark}"
    rounded: "{rounded.md}"
    padding: "10px 20px"
  button-danger:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.text-primary-dark}"
    rounded: "{rounded.md}"
    padding: "6px 12px"
---

## Overview

DesktopORZ GUI operates in **Operate Mode** (per Impeccable design guidelines). The interface prioritizes task scanability, high data density, zero-latency desktop icon management, and native Windows aesthetic cohesion.

## Colors

- **Primary Action (Blue 600 / 700):** `#2563eb` with hover `#1d4ed8` — used for primary state mutations and active commands.
- **Success / Restore (Emerald 500 / 600):** `#10b981` with hover `#059669` — dedicated to the primary value proposition: restoring desktop icon positions.
- **Danger (Red 500 / 600):** `#ef4444` with hover `#dc2626` — profile deletions and daemon halts.
- **Surfaces (Zinc Dark & Light):** Deep dark palette (`#18181b` and `#27272a`) paired with WCAG AA compliant text contrast.

## Typography

A single, unified sans-serif family (`Segoe UI`, `Inter`, `system-ui`) is used across headers, controls, and labels, preventing arbitrary font mixing. Data coordinates and CLI execution logs strictly use monospace (`Consolas`).

- Titles: 20px Bold.
- Section Headers: 14px–15px Bold.
- Body & Labels: 12px–13px Regular.
- Meta / Timestamps: 11px Muted.

## Layout

- **Header:** Sticky identity block with live status badges (Binary OK, Startup status, Hide-icons status) and theme toggle.
- **Operate Tabs:**
  - Tab 1: Profile management & interactive icon table view.
  - Tab 2: Automation cards (Inactivity daemon, Windows startup, Google Drive cloud sync).
  - Tab 3: System language & paths.
- **Console Bar:** Real-time stdout/stderr stream with exit code diagnostics, clipboard copy, and clear tools.

## Elevation & Depth

Surfaces use subtle tonal layering (card surfaces sit 1 shade above the canvas background) rather than heavy drop shadows, maintaining clean legibility.

## Shapes

- Containers and panels use `8px` rounded corners.
- Badges and controls use `6px` rounded corners.

## Components

- **Profile Card:** Click-to-select card displaying profile name, resolution, icon count, and formatted UTC timestamp.
- **Icon Table:** Filterable, scrollable list showing icon names alongside exact screen X/Y pixel coordinates.
- **Automation Card:** Self-contained configuration box with sliders, source checkboxes, and toggle buttons.

## Do's and Don'ts

- **Do:** Give immediate visual confirmation and status feedback on CLI execution.
- **Do:** Maintain strict WCAG AA contrast for text and controls.
- **Don't:** Open terminal/command prompt windows when launching or executing commands.
- **Don't:** Use decorative gradients or ambiguous icon-only buttons without clear labels.
