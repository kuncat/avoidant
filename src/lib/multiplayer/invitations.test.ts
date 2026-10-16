import { expect, test } from "vitest";
import { parseInvitation } from "./invitations";
test("short and future codes and legacy links", () => {
  for (const value of [
    "d j k g m",
    "https://kuga.run/ABCD/extra",
    "https://kuga.run@evil.test/ABCDE",
    "ABC",
    "A".repeat(33),
  ])
    expect(parseInvitation(value)).toBeNull();
  expect(parseInvitation("dJkGm")).toBe("DJKGM");
  expect(parseInvitation("kuga.run/BCDFGHJKLM")).toBe("BCDFGHJKLM");
  expect(parseInvitation("https://kuga.run/DJKGM")).toBe("DJKGM");
  const id = "00000000-0000-4000-8000-000000000001";
  expect(parseInvitation(`https://games.kuncat.com/avoidant?match=${id}`)).toBe(id);
});
