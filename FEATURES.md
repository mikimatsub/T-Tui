# Feature parity checklist

T-TUI 0.3 substantially expands the terminal client. It does **not** claim 100%
Tinder Web parity. The service has no supported public API contract for this
client, and some workflows still require Tinder Web.

| Workflow | Terminal support |
|---|---|
| Automatic browser login, session import, reconnect and refresh-token renewal | Implemented |
| Paginated inbox, previews, unread counts, saved chat drafts | Implemented |
| Search names/previews, filters, sorting, pinned chats | Implemented; local tools over the loaded inbox |
| Text chat, history pagination, incoming updates, failure recovery | Implemented |
| Profile details, native photos and carousels | Implemented |
| Discovery, likes and passes | Implemented |
| Super Like and Boost | Implemented with explicit confirmation; requires server-granted credits |
| Bio editing | Implemented; 500-character limit, explicit Save |
| Age, distance, gender preference, discovery visibility | Implemented; partial updates, explicit Save |
| Display gender on profile | Implemented |
| View own profile | Implemented from Account → Preview |
| Unmatch | Implemented; confirmation and server acknowledgement required |
| Mouse navigation, actions, scroll, composer cursor placement | Implemented across screens and modals |
| Themes | 24 palettes, live preview, apply/cancel, persistent selection |
| Reporting profiles/content | Browser handoff to official reporting instructions; no report is sent by T-TUI |
| Photo/video upload, delete/reorder, Smart Photos | Still requires Tinder Web |
| Profile interests, prompts, job/school and connected services editing | Still requires Tinder Web |
| GIFs, stickers, other media messages and reactions | Not implemented |
| Likes You, Top Picks, Explore, Double Date and other recommendation modes | Not implemented |
| Rewind, Passport, advanced premium filters and read-receipt purchases | Not implemented |
| Purchases, subscriptions, billing, account deletion | Still requires Tinder Web |
| Phone/social sign-in, verification, CAPTCHA and appeals | Automated via browser login handoff (T-TUI detects session once completed) |
| Browser push notifications, typing/presence indicators | Not implemented |

**Account → Tinder Web** opens the website for workflows outside the terminal.
Feature availability remains subject to the account and the server. A successful
local test does not prove an unofficial endpoint works for every live account.

## Validation

Automated tests use isolated local HTTP servers and a fictional offline backend.
They cover request shapes, partial account updates, rejected/ambiguous responses,
confirmation gates, duplicate-submit guards, stale responses, mouse hit regions,
Unicode cursor placement, persistence, resizing, and modal isolation.

The PTY suite sends actual SGR mouse events and keystrokes to the binary, exercises
account edits and premium actions in demo mode, verifies terminal restoration,
and checks native Sixel rendering beneath overlays. No live account mutations,
messages, swipes, unmatches, or credit consumption are part of these tests.

## API reference trail

The implementation was checked against these primary implementation sources on
2026-09-18. They are third-party clients, not a Tinder support guarantee:

- [Profile update adapter](https://github.com/Miguelo981/tinder-api/blob/main/src/adapters/update-profile.ts): partial `user` payloads sent to `/v2/profile`; distance uses miles.
- [Tinder API client](https://github.com/Miguelo981/tinder-api/blob/main/src/tinder.ts): Super Like and Boost routes.
- [Unmatch implementation](https://github.com/fbessez/Tinder/blob/master/tinder_api.py): `DELETE /user/matches/{match_id}`; this older route needs live-account validation.
- [Tinder Discovery Settings](https://www.help.tinder.com/hc/en-us/articles/115003340963-Discovery-Settings): discovery preferences and visibility behavior.
- [Tinder reporting instructions](https://www.help.tinder.com/hc/en-us/articles/115003822043-Reporting-profiles-and-content): official reporting flow used by the browser handoff.
