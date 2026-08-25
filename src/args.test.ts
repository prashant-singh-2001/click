import { describe, it, expect } from "vitest";
import { parseArgs, formatArgs } from "./args";

describe("parseArgs", () => {
  it("splits on whitespace", () => {
    expect(parseArgs("a b c")).toEqual(["a", "b", "c"]);
  });

  it("collapses runs of whitespace", () => {
    expect(parseArgs("a    b")).toEqual(["a", "b"]);
  });

  it("ignores leading and trailing whitespace", () => {
    expect(parseArgs("  a b  ")).toEqual(["a", "b"]);
  });

  it("returns an empty array for blank input", () => {
    expect(parseArgs("")).toEqual([]);
    expect(parseArgs("   ")).toEqual([]);
  });

  it("keeps a quoted run with spaces as one argument", () => {
    expect(parseArgs('"a b"')).toEqual(["a b"]);
  });

  it("treats a doubled quote inside a quoted run as one literal quote", () => {
    expect(parseArgs('"say ""hi"""')).toEqual(['say "hi"']);
  });

  it("treats an empty quoted run as an empty argument", () => {
    expect(parseArgs('""')).toEqual([""]);
  });

  it("closes an unterminated quote at end of input instead of throwing", () => {
    expect(parseArgs('--dir "C:/My')).toEqual(["--dir", "C:/My"]);
  });

  it("never treats backslash as an escape character", () => {
    expect(parseArgs("C:\\My Project\\")).toEqual(["C:\\My", "Project\\"]);
    expect(parseArgs('"C:\\My Project\\"')).toEqual(["C:\\My Project\\"]);
  });

  it("allows a quote to start mid-token, joining quoted and unquoted parts", () => {
    expect(parseArgs('--dir="C:/My Project"')).toEqual(["--dir=C:/My Project"]);
  });
});

describe("formatArgs", () => {
  it("joins bare arguments with a space", () => {
    expect(formatArgs(["a", "b"])).toBe("a b");
  });

  it("quotes an argument containing a space", () => {
    expect(formatArgs(["a b"])).toBe('"a b"');
  });

  it("quotes an empty argument", () => {
    expect(formatArgs([""])).toBe('""');
  });

  it("quotes and doubles an embedded quote", () => {
    expect(formatArgs(['say "hi"'])).toBe('"say ""hi"""');
  });

  it("returns an empty string for no arguments", () => {
    expect(formatArgs([])).toBe("");
  });
});

describe("round-trip", () => {
  const cases: string[][] = [
    ["a", "b"],
    ["a b"],
    ["C:\\My Project"],
    [""],
    ['say "hi"'],
    ["--dir", "C:/My Project", "--port", "3000"],
    [],
  ];

  it.each(cases.map((args) => [args]))(
    "parseArgs(formatArgs(%j)) equals the original",
    (args) => {
      expect(parseArgs(formatArgs(args))).toEqual(args);
    },
  );

  it("distinguishes two arguments from one argument containing a space", () => {
    expect(formatArgs(["a", "b"])).not.toBe(formatArgs(["a b"]));
  });
});
