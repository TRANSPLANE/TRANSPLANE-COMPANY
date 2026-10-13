# Four-week studio

[简体中文](studio.zh-CN.md) · [Getting started](getting-started.md)

Allocate limited hours between commissioned work and your own product. Fixed rules calculate progress, deadlines and cash. Complete all four weeks without a model; your own Agent can advise or help prepare allocations.

## Start

Create an empty simulation company using CNY, then open **Four-week studio**. Confirm **CNY 6,000** starting cash, **20 hours per week**, **CNY 800** weekly overhead and the **four-week duration** before starting. Real companies and companies with existing business records cannot use this opening.

At the end of week four, success requires all of:

- At least CNY 6,000 in cash;
- At least 24 hours invested in your own product;
- Every accepted commission completed, with no active or unfinished late orders.

## Allocate work and receive payment

Allocate positive two-hour blocks, totalling no more than 20 hours per week. Choose deliveries first, product first or a balanced plan, or specify your own allocation. Preview shows its deterministic result; confirmation saves work, payments, overhead and the next week. An idle week still incurs overhead, and unused hours do not carry forward.

The first allocation to an order accepts its commitment and deadline. Payment arrives only when all work is complete: finish by the end of its deadline week for the full fee, or later for **75%**. An unstarted offer expires at its deadline without creating an unfinished promise. Product work increases progress without automatically generating revenue.

| Commission | Available | Required work | Deadline | On-time payment |
|---|---|---:|---|---:|
| Atlas | Week 1 | 24 hours | End of week 2 | CNY 2,800 |
| Birch | Week 2 | 20 hours | End of week 3 | CNY 2,400 |
| Cedar | Week 3 | 20 hours | End of week 4 | CNY 3,600 |

For example, Atlas with four hours remaining at the end of week two becomes late. Complete those four hours in week three to receive CNY 2,100. Agent narration cannot change prices, discounts or capacity. Unavailable, delivered, expired or excessive allocations are rejected.

## Example: complete commissions on time

| Week | Allocation | Payment this week | Closing cash | Product total |
|---|---|---:|---:|---:|
| 1 | Atlas 12 hours, product 8 hours | CNY 0 | CNY 5,200 | 8 hours |
| 2 | Atlas 12 hours, product 8 hours | CNY 2,800 | CNY 7,200 | 16 hours |
| 3 | Birch 20 hours | CNY 2,400 | CNY 8,800 | 16 hours |
| 4 | Product 8 hours, idle 12 hours | CNY 0 | CNY 8,000 | 24 hours |

Atlas and Birch finish on time; Cedar is never accepted and expires at its deadline. All three product, cash and commitment conditions are met. Other routes may take more commissions, accept late-payment discounts or leave hours unused; actual allocations determine results.

After four weeks, stop at the ending and history. Pause, create another company to try another route, or continue open-ended operations with your own Agent using the current records. The player decides the next direction and goal. There is no automatic fifth week or new achievement.

## Agent integration

Read `GET /api/v1/discovery?view=operations&service=studio` and `GET /api/v1/agent-guide?topic=studio`, then use `studio.state`, `studio.preview`, `studio.start` and `studio.choose` according to deployment contracts. Confirm the opening resources first. Translate a player-authored plan into two-hour allocations, preview it and obtain confirmation before submission.

`studio.choose` atomically saves the week's choice, cash, results, settlement and next week's event. The Agent does not independently adjudicate payments or publish conflicting outcomes, nor automatically assign a new goal after four weeks. See the [Agent interface](agent-interface.en.md#four-week-studio-interface) for exact routes, permissions and retry rules.
