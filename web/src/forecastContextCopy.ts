import { localized } from "./locale";

export const zhForecastContext = {
  title: "信息来源", company: "公司记录", public: "公开资料", world: "World 互动信号", assumption: "情景假设", unknownKind: "来源类型未记录",
  observed: "观察日期", retrieved: "获取日期", expires: "有效期至", expired: "基准时已过期", afterBaseline: "基准后才取得", notRecorded: "未记录", legacy: "信息来源时间未记录",
  details: "来源详情", reference: "来源位置", method: "分析方法", limitations: "影响判断的条件", update: "复制更新推演任务", taskDetails: "查看更新任务",
  promptIntro: "请用最新实际资料更新这家公司的情景预测。", promptEntry: "服务入口", promptCompany: "公司标识", promptForecast: "原预测标识与版本", promptBaseline: "原预测基准日期",
  promptRead: "读取服务入口与 forecast.put 的当前合同，再读取本公司最新实际经营记录、目标和现有预测。明确新预测的 as_of 和基线公司版本；保留原预测，不将现在取得的信息写成当时已经知道的事实。",
  promptSources: "核对与当前问题相关的最新公开事实，保存可检查的来源位置、观察日期与获取日期。公司记录、公开资料、情景假设分开记录；只有已有授权且确实取得时才使用可选 World 互动信号，其互动身份与反馈不能作为真实客户验证。缺少来源或有效期限时明确未知。",
  promptFields: "新预测按合同保存 context_sources（kind: company|public|world|assumption；title、reference、observed_on、retrieved_on；适用时 expires_on、summary）、analysis_method 和影响结论的 limitations。说明情景假设、计算依据及与实际资金和业务的关系；用 forecast.put 保存新的预测，勿将预测收益记成实际收款。",
};
export const enForecastContext: typeof zhForecastContext = {
  title: "Information sources", company: "Company records", public: "Public sources", world: "World interaction signal", assumption: "Scenario assumption", unknownKind: "Source type not recorded",
  observed: "Observed", retrieved: "Retrieved", expires: "Valid through", expired: "Expired at the baseline", afterBaseline: "Obtained after the baseline", notRecorded: "Not recorded", legacy: "Source timing was not recorded",
  details: "Source details", reference: "Source location", method: "Analysis method", limitations: "Conditions affecting the conclusion", update: "Copy forecast update task", taskDetails: "Read update task",
  promptIntro: "Update this company's scenario forecast using the latest actual information.", promptEntry: "Service entry", promptCompany: "Company ID", promptForecast: "Original forecast ID and revision", promptBaseline: "Original forecast baseline",
  promptRead: "Read the service entry and current forecast.put contract, then this company's latest actual records, goals and forecasts. Set the new forecast's as_of and baseline company revision. Preserve the original forecast; do not present information obtained now as something known at its earlier baseline.",
  promptSources: "Check current public facts relevant to the question and retain an inspectable reference, observation date and retrieval date. Separate company records, public information and scenario assumptions. Use optional World interaction signals only when already authorized and actually obtained; their identities and responses are not real-customer validation. Keep missing sources or validity periods unknown.",
  promptFields: "Save context_sources under the current contract: kind company|public|world|assumption, title, reference, observed_on, retrieved_on, and expires_on or summary when applicable. Save analysis_method and material limitations. State scenario assumptions, calculations and their relation to actual cash and business records. Save a new forecast with forecast.put; do not record projected income as actual receipts.",
};
export const forecastContextCopy = localized(zhForecastContext, enForecastContext);
