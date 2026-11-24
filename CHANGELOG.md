# Changelog

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
