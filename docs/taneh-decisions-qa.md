# Taneh — Decision Questions and Answers

**3 November 2026 · Ali Shokoohi**

All 41 open items from the Taneh specification (Q01–Q41). 39 were asked as multiple-choice questions on 3 Nov 2026; Q08 and Q34 were already settled by the Word spec.

**Legend:** ✅ = chosen · ☆ = option recommended at the time of asking · ⚖️ = needs legal or accounting sign-off before launch

## Summary

| ID | Topic | Chosen | Followed recommendation |
| --- | --- | --- | --- |
| Q01 | Domain structure | One domain, paths | No |
| Q02 | Curtain colour | Dark velvet brown | Yes |
| Q03 | Font | Vazirmatn + its Latin | Yes |
| Q04 | Brand and motion assets | In-house designer | — |
| Q05 | Upload limits | 15 MB, JPEG/PNG/WebP/HEIC, ≥512 px | Yes |
| Q06 | Studio upload retention | On by default; unsaved 30 days | Yes |
| Q07 | Catalogue sourcing | Both | No |
| Q08 | Studio auth timing | Settled by Word spec | — |
| Q09 | Sign-in method | Mobile + SMS OTP | Yes |
| Q10 | In-flight work on deletion | Finish jobs, keep until final delete | Yes |
| Q11 ⚖️ | Tokens at final deletion | Forfeited, with explicit warning | Yes |
| Q12 | Admin access to private photos | Only on report or support ticket | Yes |
| Q13 | Unwanted but successful output | Support review, case by case | Yes |
| Q14 | External share of try-on result | File only, no public link | Yes |
| Q15 | Card limit per project | 50 cards, shown up front | Yes |
| Q16 | Failed projects | Completed, with result summary | Yes |
| Q17 | Studio output spec | 2048 px image, 1080p video ≤10 s, no watermark | Yes |
| Q18 | Output type and recolour | Per card in step 2; recolour add-on | Yes |
| Q19 | Store requirements for Share | Store name + valid link | Yes |
| Q20 | Emergency stop | Finish running, block new | Yes |
| Q21 | Widget payment | Shopper's own Taneh wallet | No |
| Q22 | Notification channels | In-app only | Yes |
| Q23 | Admin second factor | Passkey + TOTP fallback | Yes |
| Q24 ⚖️ | Revenue basis | Recognise at token spend | Yes |
| Q25 | AI auto-fallback | On, one backup per capability | Yes |
| Q26 | Bulk import format | CSV files | No |
| Q27 | Recovery targets | RPO 1 h, RTO 4 h, keep 30 days | Yes |
| Q28 | Launch throughput | 1k/day | No |
| Q29 | Retry and token return | 3 retries, instant return | Yes |
| Q30 | Liked-list name | نشون‌کرده‌ها | Yes |
| Q31 | Garment link hosts | Allowlisted stores only | Yes |
| Q32 | Auto-draft retention | 30 days | No |
| Q33 | Empty card at Create | Block until filled or removed | Yes |
| Q34 | Ring start state | Settled by Word spec | — |
| Q35 | Share withdrawal | Hidden immediately | Yes |
| Q36 | Studio pricing | Flat base + style premium | No |
| Q37 | PWA targets | Android Chrome, iOS Safari, desktop Chromium | Yes |
| Q38 | Product name | تنه / Taneh | — |
| Q39 | Garments per try-on | One garment per request | Yes |
| Q40 | Buyer personal photos | Gallery up to 5, one default | Yes |
| Q41 | Optional profile fields | Height and weight | — |

## Round 1 — blocks engineering

### Q09 — What is the final sign-in method?

- [x] ✅ **Mobile + SMS OTP** ☆ — Phone number is the account identity; simplest for the Iran market
- [ ] Mobile OTP, email linkable later — Phone is primary; email can be added and verified as a second login
- [ ] Email + password only — No SMS dependency; weaker fit for local users
- [ ] Both as equal methods — Needs safe account linking to avoid two wallets

### Q36 — How are Studio outputs priced?

- [ ] Flat per output type ☆ — One price each for image, video, recolour; simple schema and UX
- [ ] Per-style price — Each style carries its own token cost
- [x] ✅ **Flat base + style premium** — Output-type base price plus optional surcharge on premium styles

### Q33 — What happens if a card has no product image at Create?

- [x] ✅ **Block until filled or removed** ☆ — Create is disabled and empty cards are highlighted
- [ ] Generate ready cards only — Empty cards are skipped and not charged, with explicit notice
- [ ] Auto-remove empty cards — Removed on confirm, with a summary of what was dropped

### Q32 — How long is an unsaved Studio auto-draft kept?

- [ ] 7 days ☆ — From last activity; covers a working week, drafts are cheap JSON
- [ ] 24 hours — Base proposal from the original conversation
- [x] ✅ **30 days** — Most forgiving; more stale drafts to purge

## Round 2 — infrastructure and money mechanics

### Q01 — How should the domain be structured?

- [ ] One domain, subdomains ☆ — e.g. app root, studio., admin. under one parent; shared session is simple
- [x] ✅ **One domain, paths** — /tryon, /studio on one host; admin on its own subdomain
- [ ] Separate domains — Needs cross-domain SSO; more work and fragility

### Q06 — Default for "Save all uploaded photos" and TTL for unsaved Studio uploads?

- [x] ✅ **On by default; unsaved 30 days** ☆ — Sellers get a library automatically; opt-out uploads purged after 30 days
- [ ] Off by default; unsaved 30 days — Library only on purpose; temp uploads kept a month
- [ ] Off by default; unsaved 7 days — Lowest storage; temp uploads gone after a week

### Q25 — Is AI provider auto-fallback enabled at launch?

- [x] ✅ **On, one backup per capability** ☆ — Router fails over on outage; budget guard still applies
- [ ] Off at launch — Built and tested, enabled later from the console
- [ ] On for try-on only — Studio waits for manual switch

### Q29 — Retry and token-return policy for failed generations?

- [x] ✅ **3 retries, instant return** ☆ — Up to 3 internal attempts, then return tokens in the same transaction
- [ ] 5 retries, instant return — More resilience, longer worst-case wait
- [ ] Return after admin review — Slower and manual

## Round 3 — outputs, notifications, publishing, photos

### Q17 — Studio output spec at launch?

- [x] ✅ **2048 px image, 1080p video ≤10 s, no watermark** ☆ — Paid output is clean; format WebP/JPEG + MP4
- [ ] Same, with light watermark — Brand watermark, removable later by pricing tier
- [ ] 4K image, 1080p video ≤15 s — Higher provider cost and storage

### Q22 — Notification channels at launch?

- [x] ✅ **In-app only** ☆ — Per-output + all-ready events; external channels built and off
- [ ] In-app + opt-in web push — Push only after explicit permission
- [ ] In-app + SMS on project done — Studio only; costs per SMS

### Q35 — When a seller withdraws a published Share, what happens?

- [x] ✅ **Hidden immediately** ☆ — Removed from public display at once; already-downloaded copies out of our control
- [ ] Hidden within 24 hours — Queued for admin processing
- [ ] Hidden, admin approval — Withdrawal reviewed like publishing

### Q40 — Buyer personal photos: one or a gallery?

- [x] ✅ **Gallery up to 5, one default** ☆ — Fast reuse, easy switching between looks
- [ ] One saved photo — Simplest; new photo replaces or is temporary
- [ ] Unlimited gallery — More storage and privacy surface

## Round 4 — brand and buyer scope

### Q02 — Curtain colour?

- [x] ✅ **Dark velvet brown** ☆ — Matches the proposed palette and tokens already drafted
- [ ] Jade green — Needs a reworked brand token set
- [ ] Prototype both, decide in user test — Delays token freeze

### Q38 — Final product name?

- [x] ✅ **تنه / Taneh** — Keep the current working name
- [ ] تن / Tan — The Word spec's provisional name
- [ ] Not decided yet — Keep open; working name stays

### Q30 — Persian name for the buyer's Liked list?

- [x] ✅ **نشون‌کرده‌ها** ☆ — Bazaar idiom for items you've set aside; fits the voice
- [ ] پسندیده‌ها — Plain and clear
- [ ] برای بعد — Describes the action, not the items

### Q39 — Can one try-on request apply several garments of an outfit?

- [x] ✅ **One garment per request** ☆ — Matches current spec and most try-on models; cost stays clear
- [ ] Full outfit in one request — Depends on provider capability; harder pricing
- [ ] One now, outfit later — Ship single; revisit when providers support it

## Round 5 — account lifecycle, disputes, profile

### Q10 — What happens to in-flight work when an account requests deletion?

- [x] ✅ **Finish jobs, keep until final delete** ☆ — Restore within 30 days via SMS OTP brings everything back
- [ ] Cancel jobs and return tokens — Clean stop at request time
- [ ] Block deletion while jobs run — User must wait for completion

### Q11 ⚖️ — Remaining tokens at final account deletion?

- [x] ✅ **Forfeited, with explicit warning** ☆ — Shown before confirming deletion; ledger kept anonymised
- [ ] Manual IRR refund on request — Support handles it within the 30-day window
- [ ] Block deletion with balance — User must spend or forfeit first

### Q13 — Successful but unwanted output: what can the user get?

- [x] ✅ **Support review, case by case** ☆ — No automatic return; admin may return via audited adjustment
- [ ] One free regenerate — Per request; costs provider spend
- [ ] Always return on complaint — Abuse-prone

### Q41 — Which optional buyer profile fields? *(multi-select)*

- [x] ✅ **Height and weight** — Only useful if the try-on provider uses them
- [ ] Body measurements — Chest, waist, hips
- [ ] Extra photo angles — Side or full-body photos for better results
- [ ] Fit preference — Loose, regular, fitted

## Round 6 — inputs and Studio project mechanics

### Q05 — Upload limits for photos?

- [x] ✅ **15 MB, JPEG/PNG/WebP/HEIC, ≥512 px** ☆ — Covers modern phone photos; reject before any token spend
- [ ] 10 MB, same formats — Tighter; some HEIC/large photos rejected
- [ ] 25 MB, same formats — More storage and processing

### Q15 — Card limit per Studio project?

- [x] ✅ **50 cards, shown up front** ☆ — Transparent processing limit; no hidden cost
- [ ] 20 cards — Safer queue load at launch
- [ ] No cap; queue throttles — Long waits on big projects

### Q16 — Where do partly or fully failed projects go?

- [x] ✅ **Completed, with result summary** ☆ — e.g. "6 ready, 2 failed, 2 returned"; total failure flagged
- [ ] Completed only if ≥1 success — Total failures shown in a separate list
- [ ] Third "Failed" group — Breaks the two-group rule in R49

### Q18 — Where is image vs video chosen, and how is recolour priced?

- [x] ✅ **Per card in step 2; recolour add-on** ☆ — Each card picks image or video; recolour adds its own price
- [ ] Per project in step 1; recolour add-on — Whole project is image or video
- [ ] Per card; recolour free — Simpler, but provider cost unrecovered

## Round 7 — security and operations

### Q23 — Admin second factor?

- [x] ✅ **Passkey + TOTP fallback** ☆ — Phishing-resistant; recovery via another admin, audited
- [ ] TOTP only — Authenticator app codes
- [ ] SMS OTP — Weakest; SIM-swap risk

### Q20 — Emergency stop: what happens to running jobs?

- [x] ✅ **Finish running, block new** ☆ — No surprise failures for users already charged
- [ ] Cancel running, return tokens — Hard stop, e.g. runaway cost
- [ ] Admin chooses per stop — Both options in the stop dialog with impact preview

### Q27 — Recovery targets and backup retention?

- [x] ✅ **RPO 1 h, RTO 4 h, keep 30 days** ☆ — Daily full + continuous WAL / point-in-time recovery
- [ ] RPO 24 h, RTO 8 h, keep 14 days — Daily backups only; up to a day of ledger loss
- [ ] RPO 15 m, RTO 1 h, keep 90 days — Needs standby replica; higher cost

### Q14 — External share of a try-on result?

- [x] ✅ **File only, no public link** ☆ — Device share sheet or download; nothing public
- [ ] Expiring link, 7 days — Unlisted, revocable
- [ ] Permanent public link — Privacy risk for personal photos

## Round 8 — privacy, stores, integrations

### Q12 — Admin access to private user photos?

- [x] ✅ **Only on report or support ticket** ☆ — Logged, scoped to that case; consent + AI notice at first upload
- [ ] Any time for moderation, logged — Broader access, still audited
- [ ] Never; outputs only — Personal inputs invisible to admins

### Q19 — What must a store profile have before a Share can be published?

- [x] ✅ **Store name + valid link** ☆ — Checked at review; not needed to create projects
- [ ] Name, link + ownership proof — e.g. business ID or domain verification
- [ ] Store name only — Lowest friction, weakest trust

### Q21 — Who pays for try-ons through the store widget?

- [ ] Store's wallet ☆ — B2B model; shopper needs no Taneh tokens
- [x] ✅ **Shopper's own Taneh wallet** — Shopper must sign in and top up
- [ ] Free quota, then store pays — N free per shopper, rest billed to store

### Q31 — Which links can buyers paste for garment images?

- [x] ✅ **Allowlisted stores only** ☆ — Onboarded Iranian stores; lowest SSRF and licensing risk
- [ ] Any public host, hardened — SSRF guards, size and redirect caps
- [ ] Direct image URLs only — No product-page parsing

## Round 9 — brand assets, catalogue, finance, capacity

### Q03 — Font family?

- [x] ✅ **Vazirmatn + its Latin** ☆ — Open licence (OFL), good FA/EN harmony, Tahoma fallback
- [ ] Vazirmatn + Inter for Latin — Stronger Latin UI, two families to tune
- [ ] Licensed commercial font — e.g. Yekan Bakh; licence cost and web terms

### Q07 — How do garments get into the buyer catalogue?

- [ ] Brands submit with licence ☆ — Via console or Excel import; signed image-use agreement
- [ ] Taneh team curates — Team sources and licenses each item
- [x] ✅ **Both** — Curated launch set, then brand self-serve

### Q24 ⚖️ — Revenue and profit basis for reports?

- [x] ✅ **Recognise at token spend** ☆ — Unspent tokens are a liability; AI cost at invoice-date FX
- [ ] Recognise at purchase — Simpler, overstates early revenue
- [ ] Report both views — Cash in vs earned, side by side

### Q28 — Launch throughput target for load testing?

- [ ] 10k generations/day, test at 2x ☆ — Peak ~50 concurrent jobs; scale workers after
- [x] ✅ **1k/day** — Soft launch scale
- [ ] 100k/day — Needs provider quotas and budget upfront

## Round 10 — final three

### Q04 — Who produces the logo, Latin wordmark and curtain/hand motion assets?

- [x] ✅ **In-house designer** — Deliver Lottie/SVG + reduced-motion stills
- [ ] External agency — Brief from §12 tokens and voice guide
- [ ] Not decided yet — Keep Q04 open

### Q26 — Bulk import layout?

- [ ] One sheet per entity ☆ — Styles, Models, Garments, Relations; stable IDs, FA/EN names, file names
- [ ] One flat sheet — Row per item; relations as ID lists in a cell
- [x] ✅ **CSV files instead** — Simpler parsing, less friendly for content ops

### Q37 — PWA target environments?

- [x] ✅ **Android Chrome, iOS Safari, desktop Chromium** ☆ — Covers most users; no offline generation
- [ ] Android Chrome only — iOS gets plain web
- [ ] All major browsers incl. Firefox — Firefox desktop has no install support

## Settled by the Word spec (not asked)

### Q08 — When does Studio require sign-in?

- [x] ✅ **Lobby is public; all three build entries (Create Project, ring model, sample project) require sign-in before P16 or P17.** Replaces the earlier guest-until-Generate behaviour.

### Q34 — Start state when entering from the model ring

- [x] ✅ **After sign-in, P16 opens with all of that model's styles pre-selected;** the seller can remove some or all, add more, or add styles from other models.
