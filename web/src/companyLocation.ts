import type { Company } from "./types";

/** URL company IDs are hints; only the authorized company list may select one. */
export function companyFromLocation(companies: Company[], current: string, requested: string | null) {
  return (companies.find((company) => company.id === current)
    || companies.find((company) => company.id === requested)
    || companies[0])?.id || "";
}
