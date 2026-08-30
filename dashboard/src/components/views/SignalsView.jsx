import React from 'react';
import { useTrading } from '../../context/TradingContext';
import DataTable from '../common/DataTable';
import StatusBadge from '../common/StatusBadge';
import StatCard from '../common/StatCard';
import { 
  ShieldAlert, 
  Radio, 
  Newspaper, 
  Flame, 
  AlertOctagon, 
  CheckCircle2, 
  Lock, 
  ShieldCheck, 
  DollarSign 
} from 'lucide-react';

export default function SignalsView() {
  const { 
    signals, 
    catalysts, 
    engineState, 
    triggerKillSwitch 
  } = useTrading();

  const signalColumns = [
    {
      header: 'Time',
      accessor: 'time',
      width: '90px',
      render: (_, val) => <span className="font-mono text-slate-400 text-[11px]">{val}</span>
    },
    {
      header: 'Signal ID',
      accessor: 'id',
      width: '90px',
      render: (_, val) => <span className="font-mono font-bold text-slate-200 text-[11px]">{val}</span>
    },
    {
      header: 'Venue',
      accessor: 'venue',
      width: '110px',
      render: (_, val) => <StatusBadge status={val} variant="blue" size="sm" />
    },
    {
      header: 'Target Market',
      accessor: 'marketTitle',
      render: (_, val) => <span className="font-medium text-slate-100">{val}</span>
    },
    {
      header: 'Type',
      accessor: 'type',
      width: '140px',
      render: (_, val) => (
        <span className="inline-flex items-center gap-1 font-mono text-[11px] font-bold text-amber-400">
          <Flame className="w-3.5 h-3.5" />
          <span>{val}</span>
        </span>
      )
    },
    {
      header: 'Z-Score',
      accessor: 'zScore',
      align: 'right',
      width: '90px',
      render: (_, val) => <span className="font-mono font-bold text-emerald-400">+{val.toFixed(1)}σ</span>
    },
    {
      header: 'Sweep Volume',
      accessor: 'sizeUsd',
      align: 'right',
      width: '120px',
      render: (_, val) => <span className="font-mono font-bold text-slate-100">${val.toLocaleString()}</span>
    },
    {
      header: 'Headline Context',
      accessor: 'headline',
      render: (_, val) => <span className="text-slate-400 text-xs truncate max-w-xs">{val}</span>
    }
  ];

  const ddPct = (engineState.dailyDrawdown / engineState.maxDrawdownLimit) * 100;

  return (
    <div className="space-y-6">
      {/* Risk Governor Circuit Breaker Section */}
      <div className="card p-5 space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-[#1a2538] pb-3">
          <div className="space-y-0.5">
            <div className="flex items-center gap-2">
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
              <h2 className="text-xs font-bold text-slate-100 uppercase tracking-wider">
                Institutional Risk Governor & Circuit Breakers
              </h2>
            </div>
            <p className="text-xs text-slate-400">Hard mathematical limits enforced prior to every order dispatch</p>
          </div>

          <div className="flex items-center gap-2">
            <span className={`text-[11px] font-semibold px-2 py-0.5 rounded border ${
              engineState.isKillSwitchActive
                ? 'bg-rose-500/10 text-rose-400 border-rose-500/30'
                : 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30'
            }`}>
              {engineState.isKillSwitchActive ? 'HALTED (KILL SWITCH ACTIVE)' : 'ALL RAILS NORMAL'}
            </span>
          </div>
        </div>

        {/* Risk Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 text-xs">
          <div className="bg-[#0a0f1a] p-3 rounded border border-[#1a2538]">
            <span className="text-slate-500 block text-[10px] uppercase font-semibold">Peak Daily Drawdown</span>
            <div className="text-lg font-bold font-mono text-slate-100 mt-1">${engineState.dailyDrawdown.toFixed(2)}</div>
            <div className="text-[10px] text-slate-400 mt-0.5">Limit: ${engineState.maxDrawdownLimit.toFixed(2)} (Hard Stop)</div>
          </div>

          <div className="bg-[#0a0f1a] p-3 rounded border border-[#1a2538]">
            <span className="text-slate-500 block text-[10px] uppercase font-semibold">Max Position Cap</span>
            <div className="text-lg font-bold font-mono text-slate-100 mt-1">${engineState.maxPositionCap.toFixed(2)}</div>
            <div className="text-[10px] text-slate-400 mt-0.5">Single-market ceiling</div>
          </div>

          <div className="bg-[#0a0f1a] p-3 rounded border border-[#1a2538]">
            <span className="text-slate-500 block text-[10px] uppercase font-semibold">Slippage Guard</span>
            <div className="text-lg font-bold font-mono text-slate-100 mt-1">1.5% Max</div>
            <div className="text-[10px] text-slate-400 mt-0.5">Rejects wide taker fills</div>
          </div>

          <div className="bg-[#0a0f1a] p-3 rounded border border-[#1a2538]">
            <span className="text-slate-500 block text-[10px] uppercase font-semibold">Dead-Man's Switch</span>
            <div className="text-lg font-bold font-mono text-emerald-400 mt-1">5,000ms WS Latched</div>
            <div className="text-[10px] text-slate-400 mt-0.5">Auto-cancels open orders on drop</div>
          </div>
        </div>
      </div>

      {/* OddsQ Institutional Whale Stream */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-xs font-bold text-slate-100 uppercase tracking-wider flex items-center gap-2">
              <Radio className="w-4 h-4 text-amber-400" />
              <span>OddsQ Institutional Signal Stream (z ≥ 3.0 Spikes & Sweeps)</span>
            </h3>
            <p className="text-xs text-slate-400 mt-0.5">Real-time volume imbalance alerts ingested via authenticated HMAC webhook</p>
          </div>
        </div>

        <DataTable
          columns={signalColumns}
          data={signals}
          emptyMessage="No institutional signals detected in current window."
        />
      </div>

      {/* SLM Breaking News Catalysts */}
      <div className="card p-5 space-y-4">
        <div className="flex items-center justify-between border-b border-[#1a2538] pb-2">
          <div className="flex items-center gap-2">
            <Newspaper className="w-4 h-4 text-purple-400" />
            <h3 className="text-xs font-bold text-slate-100 uppercase tracking-wider">
              SLM Breaking News Catalyst Feed & Inferred Shifts
            </h3>
          </div>
          <span className="text-[11px] font-mono text-slate-400">Local Small Language Model Parser</span>
        </div>

        <div className="space-y-3">
          {catalysts.map((cat) => (
            <div key={cat.id} className="p-3 bg-[#0a0f1a] rounded border border-[#1a2538] flex items-start justify-between gap-4">
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-mono text-slate-500">{cat.time}</span>
                  <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded bg-slate-800 text-slate-300">
                    {cat.source}
                  </span>
                </div>
                <h4 className="font-semibold text-slate-100 text-xs">{cat.headline}</h4>
              </div>

              <div className="text-right flex-shrink-0 font-mono">
                <div className="text-xs font-bold text-cyan-400">{cat.impliedShift}</div>
                <div className="text-[10px] text-slate-500">{(cat.confidence * 100).toFixed(0)}% Conf</div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
