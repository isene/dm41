# Changelog

## 0.4.0 (2026-11-24)

- `B` copies the calculator's whole disk to `~/.dm41/backups/DATE_TIME`.
- `l` loads a program from a `.raw` file or a listing into the setup.
- Editing a program the setup has gives the setup the new version; Space
  puts your edited version in.
- X-Memory text files: `l` loads a text file, one record per line, and
  `e` edits one in scribe.

## 0.3.0 (2026-11-24)

- A setup can carry programs, X-Memory files and flags: Space in Programs,
  X-Memory or Flags puts each in or out of the picked setup's memory.
- Such a setup is written as a full backup in `BACKUP`, which the
  calculator loads with Settings menu, Restore from Backup.
- `n` on a full backup copies its memory too.

## 0.2.3 (2026-11-24)

- Program listings take the colours of hp41.vim: labels gold, jumps purple,
  stops red, tests orange, registers and flags blue, text green, numbers cyan.
- A minus sign is no longer shown as a number.

## 0.2.2 (2026-11-24)

- dm41 mounts the calculator's disk when it is plugged in and unmounts it on
  quit, through a sudoers line (see the README).
- `--mount` and `--unmount` on the command line.

## 0.2.1 (2026-11-24)

- Black panes.
- The top bar keeps its grey after the picked tab.

## 0.2.0 (2026-11-24)

- Modules tab: every .MOD file on the calculator's disk, with its XROM
  numbers and function names.
- Setups tab: the module lists (.m41) on the disk. `n` makes a new setup,
  Space in Modules plugs a module in or out, `s` copies it to the calculator.
- Listings name the functions of the setup's modules, read from their files.
- A copy of the disk's modules and setups is kept in ~/.dm41/disk.
- X<I>Y, the DM41X's extra X-Function, is known by name.

## 0.1.0 (2026-11-24)

- Reads a DM41X state file (.d41) from the calculator's disk, or the last copy.
- Programs as listings, with the 41CX X-Functions and Time functions by name.
- The stack, ALPHA and the data registers; all 56 flags with what they do.
- X-Memory files: programs, data and text.
- Edit a program in scribe, run it in rpnx, save it as a .raw file the
  calculator loads, copied to its PROG folder when the disk can be written.
- `--list`, `--raw`, `--xm` and `--encode` on the command line.
