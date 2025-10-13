# OpenGuild Roadmap

This roadmap mirrors the high-level milestones defined in `../BRIEF.md`. Each milestone tracks its deliverables and current status.

| Milestone | Focus                                          | Status       | Notes                                                                                                     |
| --------- | ---------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------- |
| M0        | Single-server text MVP                         | In progress | Config loader, metrics, messaging CRUD/WebSocket fan-out, refresh-token lifecycle, `/users/register` + CLI seeding. |
| M1        | End-to-end encryption for DMs/private channels | Not started | Integrate MLS key management, device onboarding.                                                          |
| M2        | Text federation                                | Not started | Implement signed-event DAG, S2S API, state resolution.                                                    |
| M3        | Bots & webhooks                                | Not started | OAuth2 client credentials, slash commands, outbound webhooks.                                             |
| M4        | Single-server voice                            | Not started | SFU deployment, signaling, SFrame integration.                                                            |
| M5        | Federated voice                                | Not started | SFU peering, membership-aware routing.                                                                    |
| M6        | Polish & extras                                | Not started | Threads, presence, moderation enhancements.                                                               |

Update this document as milestones progress.
