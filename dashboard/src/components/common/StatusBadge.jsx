import React from 'react';

export default function StatusBadge({ status, variant = 'neutral', size = 'md' }) {
  const variants = {
    emerald: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/25',
    rose: 'bg-rose-500/10 text-rose-400 border-rose-500/25',
    amber: 'bg-amber-500/10 text-amber-400 border-amber-500/25',
    blue: 'bg-blue-500/10 text-blue-400 border-blue-500/25',
    purple: 'bg-purple-500/10 text-purple-400 border-purple-500/25',
    cyan: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/25',
    neutral: 'bg-slate-800 text-slate-300 border-slate-700'
  };

  const sizeClasses = size === 'sm' ? 'text-[10px] px-1.5 py-0.5' : 'text-[11px] px-2 py-0.5';

  return (
    <span className={`inline-flex items-center gap-1 rounded font-semibold border ${variants[variant] || variants.neutral} ${sizeClasses}`}>
      {status}
    </span>
  );
}
