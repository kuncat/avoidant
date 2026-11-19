import { page } from "vitest/browser";
import { describe, expect, it, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { createRawSnippet } from "svelte";
import AvoidantGame from "./AvoidantGame.svelte";
import type { CompetitiveConfig } from "$lib/multiplayer/types";

const OPEN_GAME = "10000000-0000-4000-8000-000000000001";
const account: CompetitiveConfig["account"] = {
  login: async () => {},
  logout: async () => {},
  completeLogin: async () => {},
  token: async () => "token",
  account: async () => ({ id: "20000000-0000-4000-8000-000000000001", capabilities: [] }),
};
const competitive: CompetitiveConfig = { serverUrl: "http://127.0.0.1:9", account };

/** The embedder's open-games list: one game, joined through the callback the menu passes in. */
const matchmaking = (joined: (id: string) => void) =>
  createRawSnippet<[string, (invitation: string) => void]>((locale, join) => ({
    render: () =>
      `<section data-locale="${locale()}"><h2>Open games</h2><button type="button">Join open game</button></section>`,
    setup: (node) => {
      node.querySelector("button")!.onclick = () => {
        joined(OPEN_GAME);
        join()(OPEN_GAME);
      };
    },
  }));
const joinBadge = createRawSnippet<[string]>(() => ({
  render: () => `<span data-testid="open-count">3</span>`,
}));

describe("AvoidantGame start menu", () => {
  it("keeps open games off the start menu and shows them after Join Game", async () => {
    render(AvoidantGame, { competitive, matchmaking: matchmaking(() => {}), joinBadge });
    const join = page.getByRole("button", { name: /^Join Game/ });
    await expect.element(join).toBeEnabled();
    await expect.element(page.getByRole("heading", { name: "Open games" })).not.toBeInTheDocument();

    await join.click();
    await expect.element(page.getByRole("heading", { name: "Open games" })).toBeVisible();
    await expect.element(page.getByLabelText("Ticket or Invitation URL")).toBeVisible();

    await page.getByRole("button", { name: "Back" }).click();
    await expect.element(page.getByRole("heading", { name: "Open games" })).not.toBeInTheDocument();
  });

  it("renders the embedder's count inside the Join Game button", async () => {
    render(AvoidantGame, { competitive, joinBadge });
    const join = page.getByRole("button", { name: /^Join Game/ });
    await expect.element(join.getByTestId("open-count")).toHaveTextContent("3");
  });

  it("shows a plain Join Game button when the embedder gives no count", async () => {
    render(AvoidantGame, { competitive });
    await expect
      .element(page.getByRole("button", { name: "Join Game", exact: true }))
      .toBeEnabled();
    await expect.element(page.getByTestId("open-count")).not.toBeInTheDocument();
  });

  it("joins a listed game in place, leaving the menu for the game's lobby", async () => {
    const joined = vi.fn();
    render(AvoidantGame, { competitive, matchmaking: matchmaking(joined) });
    await page.getByRole("button", { name: /^Join Game/ }).click();
    await page.getByRole("button", { name: "Join open game" }).click();
    expect(joined).toHaveBeenCalledWith(OPEN_GAME);
    // The lobby replaces the menu, which is hidden from every reader; the page is not reloaded.
    await expect.element(page.getByLabelText("Ticket or Invitation URL")).not.toBeInTheDocument();
    await expect.element(page.getByRole("button", { name: /^Join Game/ })).not.toBeInTheDocument();
  });

  it("does not render open games without multiplayer", async () => {
    render(AvoidantGame, { accountClient: account, matchmaking: matchmaking(() => {}), joinBadge });
    await expect.element(page.getByRole("button", { name: /^Join Game/ })).toBeDisabled();
    await expect.element(page.getByTestId("open-count")).not.toBeInTheDocument();
  });
});
