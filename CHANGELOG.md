# Changelog

## 0.1.0 (2026-11-24)

- Reads a DM41X state file (.d41) from the calculator's disk, or the last copy.
- Programs as listings, with the 41CX X-Functions and Time functions by name.
- The stack, ALPHA and the data registers; all 56 flags with what they do.
- X-Memory files: programs, data and text.
- Edit a program in scribe, run it in rpnx, save it as a .raw file the
  calculator loads, copied to its PROG folder when the disk can be written.
- `--list`, `--raw`, `--xm` and `--encode` on the command line.
