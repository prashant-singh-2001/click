// Parses/formats a command-line-style argument string for the args field in
// ActionEditor. Deliberately NOT CommandLineToArgvW's grammar: backslash is
// always literal, never an escape. That real Windows rule (2n backslashes +
// quote toggles, 2n+1 escapes the quote) makes every ordinary Windows path
// need zero special-casing here, which is the whole point of this fix
// (issue #8) — `C:\My Project\` should never need typing backslashes twice.
//
// Only `"` is special: it toggles a "quoted" mode, and `""` while quoted is
// one literal quote. Quotes can appear anywhere in a token, not just at its
// start — `--dir="C:/My Project"` parses as one argument, the same way it
// would in npm/webpack-style CLI conventions.

/** Splits a command-line-style string into discrete arguments. Whitespace
 *  outside quotes separates arguments; runs of whitespace collapse. An
 *  unterminated quote closes at end of input rather than throwing — this
 *  runs on every keystroke, so half-typed input is the normal case, not an
 *  error. */
export function parseArgs(input: string): string[] {
  const args: string[] = [];
  let current = "";
  let inToken = false;
  let inQuotes = false;

  for (let i = 0; i < input.length; i++) {
    const ch = input[i];

    if (inQuotes) {
      if (ch === '"') {
        if (input[i + 1] === '"') {
          current += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        current += ch;
      }
      continue;
    }

    if (ch === '"') {
      inQuotes = true;
      inToken = true;
    } else if (/\s/.test(ch)) {
      if (inToken) {
        args.push(current);
        current = "";
        inToken = false;
      }
    } else {
      current += ch;
      inToken = true;
    }
  }

  if (inToken) {
    args.push(current);
  }
  return args;
}

/** Inverse of `parseArgs`: quotes an argument if it's empty or contains
 *  whitespace or a `"` (doubling any embedded quote), leaves it bare
 *  otherwise. Round-trips losslessly with `parseArgs` — in particular,
 *  `["a", "b"]` and `["a b"]` format to visibly different strings. */
export function formatArgs(args: string[]): string {
  return args.map(formatOneArg).join(" ");
}

function formatOneArg(arg: string): string {
  if (arg.length > 0 && !/[\s"]/.test(arg)) {
    return arg;
  }
  return `"${arg.replace(/"/g, '""')}"`;
}
