import React from 'react';

export default function StatCard({
  label,
  value,
  subtitle,
  trend,
  trendPositive,
  icon: Icon,
  badge
}) {
  return (
    <div className="card p-4 flex flex-col justify-between">
      <div className="flex items-center justify-between gap-2 mb-2">
        <span className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
          {label}
        </span>
        {badge && (
          <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20">
            {badge}
          </span>
        )}
        {Icon && <Icon className="w-4 h-4 text-slate-500" />}
      </div>

      <div>
        <div className="text-xl font-extrabold text-slate-100 font-mono tracking-tight">
          {value}
        </div>

        {(subtitle || trend) && (
          <div className="flex items-center gap-1.5 mt-1 text-[11px]">
            {trend && (
              <span className={`font-bold font-mono ${trendPositive ? 'text-emerald-400' : 'text-rose-400'}`}>
                {trend}
              </span>
            )}
            {subtitle && <span className="text-slate-500">{subtitle}</span>}
          </div>
        )}
      </div>
    </div>
  );
}
