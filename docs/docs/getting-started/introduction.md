# Introduction

Iron OS is an opinionated operating system builder based on Buildroot.
Its main use case at the moment is running a kiosk web application.

## Key features

- power outage resistant automatic updates and slow network speeds.
- Faster updates (less bandwidth) via custom packages allowing the image to be updated less often.
- Secure updates as they always need to signed and verified before installation.
- Chromium can easely be used for running local web application in kiosk mode.
- Rust crate for controlling various aspects of the system.

## Overview

The project is split into four main parts (packages server, buildroot, service
bootloader and service):

### Packages server

The first part you need is a package server, which provides packages and images
to your devices, so they can update themselves.
This does not collect any data from the devices, that is intended to be done by
a user application.
What it allows is updates based on a whitelist or limited update volumes for
a controlled rollout.

### Buildroot

In the buildroot folder you can find scripts and configurations to build the
rootfs and the kernel.

Buildroot configuration is split between boards and products.

#### Boards

Boards are pre configured buildroot configurations. At the moment four boards exist:
- intel
- intel-headless
- pi4
- pi4-headless

#### Products

Products are always project dependent and not included in this repository.
They configure which packages to include and which package to start.
It also includes which package server and which channel to use.

### Service bootloader

The service bootloader is a small systemd service which runs as root and is started
once the os has booted. It is responsible for starting the main service, weston
(the compositor) and doing some hardware configuration. It also executes image updates
and os installations from a usb stick.

### Service

Service is the main application running on iron. It provides the service api
to control various system parts. It is also responsible for starting chromium and
managing the open pages.

Some features the service api provides:
- system infos (like cpu load, memory usage, etc.)
- the ability to install the os on another disk (intended for usb sticks)
- modifying packages and requesting updates
- controlling the display like turning it off or dimming it.
- managing wifi or gsm (sim card) connections
- taking screenshots and sending mouse inputs
