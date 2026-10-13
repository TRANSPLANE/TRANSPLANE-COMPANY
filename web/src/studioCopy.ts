import { localized } from "./locale";

export const zhStudio = {
  title: "四周工作室", narrative: "Agent 叙事", simulation: "模拟经营", start: "确认资源，开始四周经营",
  starting: "正在开局", startHelp: "你有四周时间。在订单交付与自己的产品之间分配每周 20 小时。",
  startingCash: "启动资金", capacity: "每周时间", overhead: "每周固定开销", goal: "第四周目标", cash: "现金", product: "产品进度",
  cashTarget: "现金至少", productTarget: "产品至少", commitmentsTarget: "已接受的订单全部结清", hours: "小时", week: "第 {week} 周", weeks: "四周",
  deadline: "交付期限", remaining: "尚需", payment: "按期交付收款", order: "订单", orders: "订单与期限", offered: "待接", active: "进行中", late: "逾期", delivered: "已交付", expired: "机会已过",
  lateRule: "逾期交付按原价的 {percent}% 收款。", choose: "本周怎么安排？", benefit: "争取", tradeoff: "代价", custom: "自己分配时间",
  plan: "本周时间安排", unused: "留空", planned: "已分配", preview: "预览计划", previewing: "正在核算预览", previewTitle: "按此计划，本周会发生", settle: "确认并结算本周", settling: "正在结算",
  next: "进入第 {week} 周", recap: "回看第 {week} 周", result: "第 {week} 周结算", before: "本周开始", after: "本周结束", change: "现金变化", productChange: "产品增加",
  worked: "本周投入", received: "本周收款", savedTime: "未使用时间", afterHours: "结算后尚需", resultBasis: "核算依据", weeklyCost: "本周固定开销",
  chapterEnd: "四周结束了", success: "目标达成", incomplete: "目标未全部达成", met: "达成", notMet: "未达成", unknown: "待补充", history: "每周回看", continueStory: "进入 Agent 叙事",
  rules: "规则", stop: "停在这里", stopped: "已停在篇章结尾", readOnly: "开始和结算需经营、资金、报告和目标写入权限。",
  realUnavailable: "请新建模拟公司进入四周工作室。", currencyUnavailable: "四周工作室使用 CNY。请新建 CNY 模拟公司。", existingUnavailable: "此公司不符合空白开局条件。请新建模拟公司开始。",
  loadError: "无法读取四周工作室。", writeError: "未确认结算结果。输入仍然保留，可用原请求重试。", changedSession: "访问会话已变化，请重新读取公司。", retry: "重试",
  hoursInvalid: "每项使用 0 到 20 的偶数小时，总计不超过 20 小时。", previewRequired: "先预览当前时间安排，再确认结算。", pendingChoice: "选择一个方案，或自己分配时间。",
  scene1: "第一份委托摆在桌上。接单能补充现金，你的产品也需要这二十个小时。", scene2: "新的委托来了。上周答应的交付还在，先看看剩余工时。", scene3: "最后一份委托开放了。只剩两周，接下它就要重新安排产品与交付。", scene4: "最后一周。兑现承诺、完成产品、守住现金，剩下的二十小时由你安排。",
  deliverTitle: "先兑现交付", deliverBenefit: "优先交付委托，完成后收款。", deliverTradeoff: "自己的产品可能要等到下周。", productTitle: "先做自己的产品", productBenefit: "优先积累产品进度。", productTradeoff: "暂不接新委托，现金依靠现有储备。", balanceTitle: "两边都推进", balanceBenefit: "产品最多安排八小时，再处理委托。", balanceTradeoff: "交付可能分到多周，期限会更紧。",
  acceptRule: "首次结算订单工时，就接下该订单的交付期限。", offerRule: "未开工的订单到期会失效，不形成交付承诺。",
  onTimePayment: "按期交付，全额收款。", latePayment: "迟交，按原价的 {percent}% 收款。", lateUnfinished: "交期已到，订单尚未完成。",
};
export const enStudio: typeof zhStudio = {
  title: "Four-week studio", narrative: "Agent narrative", simulation: "Simulation", start: "Confirm resources and start",
  starting: "Starting", startHelp: "You have four weeks. Split 20 hours each week between customer orders and your own product.",
  startingCash: "Starting cash", capacity: "Weekly time", overhead: "Weekly overhead", goal: "Week four goal", cash: "Cash", product: "Product progress",
  cashTarget: "Cash of at least", productTarget: "Product of at least", commitmentsTarget: "Settle all accepted orders", hours: "hours", week: "Week {week}", weeks: "Four weeks",
  deadline: "Due", remaining: "Remaining", payment: "On-time delivery payment", order: "Order", orders: "Orders and deadlines", offered: "Offered", active: "In progress", late: "Late", delivered: "Delivered", expired: "Offer expired",
  lateRule: "Late delivery pays {percent}% of the original price.", choose: "How will you spend this week?", benefit: "Gain", tradeoff: "Trade-off", custom: "Allocate time yourself",
  plan: "This week's time", unused: "Unassigned", planned: "Allocated", preview: "Preview plan", previewing: "Calculating preview", previewTitle: "This plan's exact weekly outcome", settle: "Confirm and settle this week", settling: "Settling",
  next: "Enter week {week}", recap: "Review week {week}", result: "Week {week} result", before: "Week start", after: "Week end", change: "Cash change", productChange: "Product added",
  worked: "Work this week", received: "Payment this week", savedTime: "Unused time", afterHours: "Remaining after settlement", resultBasis: "Calculation basis", weeklyCost: "This week's overhead",
  chapterEnd: "The four weeks have ended", success: "Goal achieved", incomplete: "Not all goals achieved", met: "Met", notMet: "Not met", unknown: "Unknown", history: "Weekly history", continueStory: "Enter Agent narrative",
  rules: "Rules", stop: "Pause here", stopped: "Paused at the chapter's end", readOnly: "Starting and settling need company, finance, report and goal write access.",
  realUnavailable: "Create a simulation company to enter the four-week studio.", currencyUnavailable: "The studio uses CNY. Create a CNY simulation company.", existingUnavailable: "This company does not meet the blank-start conditions. Create a simulation company to start.",
  loadError: "Could not load the four-week studio.", writeError: "The settlement has not been confirmed. Your input is preserved; retry the same request.", changedSession: "The access session changed. Read the company again.", retry: "Retry",
  hoursInvalid: "Use even numbers from 0 to 20 hours per item, with no more than 20 hours in total.", previewRequired: "Preview the current allocation before settling it.", pendingChoice: "Choose a plan or allocate time yourself.",
  scene1: "Your first commission is on the desk. Paid work can fund the studio, but your own product needs the same twenty hours.", scene2: "A new commission arrives. Last week's promises still need delivering; check your remaining hours.", scene3: "The final commission opens. Two weeks remain. Taking it means rescheduling your product and deliveries.", scene4: "The final week: deliver your promises, finish your product and preserve cash. You have twenty hours to allocate.",
  deliverTitle: "Deliver commitments first", deliverBenefit: "Prioritize commissions and collect payment on delivery.", deliverTradeoff: "Your own product may wait until next week.", productTitle: "Build your own product first", productBenefit: "Prioritize product progress.", productTradeoff: "Skip new commissions and rely on existing cash.", balanceTitle: "Move both forward", balanceBenefit: "Give the product up to eight hours, then work on commissions.", balanceTradeoff: "Delivery may stretch across weeks and tighten deadlines.",
  acceptRule: "Settling the first hours on an order accepts its delivery deadline.", offerRule: "An unstarted offer expires at its deadline without creating a promise.",
  onTimePayment: "Delivered on time, paid in full.", latePayment: "Delivered late, paid {percent}% of the original price.", lateUnfinished: "The deadline has arrived and the order is unfinished.",
};
export const studioCopy = localized(zhStudio, enStudio);
export function studioWeek(template: string, week: number) { return template.replace("{week}", String(week)); }
