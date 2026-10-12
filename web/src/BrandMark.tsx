export function BrandMark({ size = 40 }: { size?: number }) {
  return <picture className="company-brand-mark" aria-hidden="true">
    <source media="(prefers-color-scheme: dark)" srcSet="/assets/company-mark-dark.svg" />
    <img src="/assets/company-mark-light.svg" alt="" width={size} height={size} />
  </picture>;
}
