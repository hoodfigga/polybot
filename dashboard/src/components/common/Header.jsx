import React from 'react';
import { useTrading } from '../../context/TradingContext';
import { 
  LayoutDashboard, 
  TrendingUp, 
  Layers, 
  Server, 
  ShieldAlert, 
  Zap, 
  AlertOctagon, 
  CheckCircle2, 
  Radio,
  RotateCcw
} from 'lucide-react';

export default function Header() {
  const { 
    currentView, 
    setCurrentView, 
    engineState, 
    toggleAutoTrading, 
    triggerKillSwitch,
    resetStats
  } = useTrading();

  const navItems = [
    { id: 'overview', label: 'Overview', icon: LayoutDashboard },
    { id: 'markets', label: 'Markets & Books', icon: TrendingUp },
    { id: 'positions', label: 'Positions & Trades', icon: Layers },
    { id: 'fleet', label: 'AWS Fleet (6 Nodes)', icon: Server },
    { id: 'signals', label: 'Signals & Risk', icon: ShieldAlert },
  ];

  return (
    <header className="bg-[#0b101c] border-b border-[#1a2538] sticky top-0 z-50">
      {/* Top Meta Bar */}
      <div className="px-4 lg:px-6 py-2.5 flex items-center justify-between border-b border-[#141d2d] text-xs">
        {/* Brand & Connection Status */}
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 rounded bg-blue-600 flex items-center justify-center font-black text-white text-sm tracking-tight">
            T
          </div>
          <div>
            <div className="font-bold text-slate-100 flex items-center gap-2 text-xs tracking-tight">
              <span>TABULA TRADER</span>
              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                {engineState.status}
              </span>
            </div>
          </div>
        </div>

        {/* Global Summary Ticker */}
        <div className="hidden md:flex items-center gap-6 text-[11px] font-mono">
          <div className="flex items-center gap-1.5 text-slate-400">
            <span>TOTAL CAPITAL:</span>
            <strong className="text-slate-100">${engineState.totalEquity.toLocaleString('en-US', { minimumFractionDigits: 2 })}</strong>
          </div>

          <div className="flex items-center gap-1.5 text-slate-400">
            <span>REALIZED P&L:</span>
            <strong className="text-emerald-400">
              +${engineState.realizedPnl.toLocaleString('en-US', { minimumFractionDigits: 2 })} ({engineState.roiPct >= 0 ? '+' : ''}{engineState.roiPct.toFixed(1)}%)
            </strong>
          </div>

          <div className="flex items-center gap-1.5 text-slate-400">
            <span>WIN RATE:</span>
            <strong className="text-slate-100">{engineState.winRate.toFixed(1)}%</strong>
          </div>

          <div className="flex items-center gap-1.5 text-slate-400">
            <span>FLEET:</span>
            <strong className="text-cyan-400">{engineState.activeNodesCount} Nodes ({engineState.avgLatencyMs}ms)</strong>
          </div>
        </div>

        {/* Global Actions */}
        <div className="flex items-center gap-2">
          {/* Reset Stats */}
          <button
            onClick={resetStats}
            className="btn btn-sm bg-slate-800 hover:bg-slate-700 text-slate-300 border-slate-700"
            title="Reset all dashboard statistics, P&L, and trade history"
          >
            <RotateCcw className="w-3 h-3" />
            <span className="hidden sm:inline">Reset Stats</span>
          </button>

          {/* Auto-Pilot Toggle */}
          <button
            onClick={toggleAutoTrading}
            className={`btn btn-sm ${
              engineState.autoTradingEnabled ? 'btn-emerald' : 'bg-slate-800 text-slate-400 border-slate-700'
            }`}
            title={engineState.autoTradingEnabled ? 'Auto-Trading Active' : 'Auto-Trading Paused'}
          >
            <Zap className="w-3 h-3" />
            <span>{engineState.autoTradingEnabled ? 'Auto-Pilot On' : 'Auto-Pilot Paused'}</span>
          </button>

          {/* Kill Switch */}
          <button
            onClick={triggerKillSwitch}
            disabled={engineState.isKillSwitchActive}
            className="btn btn-sm btn-danger"
            title="Emergency Kill Switch: Cancels all open orders and halts trading immediately"
          >
            <AlertOctagon className="w-3 h-3" />
            <span className="hidden sm:inline">{engineState.isKillSwitchActive ? 'Halted' : 'Kill Switch'}</span>
          </button>
        </div>
      </div>

      {/* Main Navigation Tabs */}
      <div className="px-4 lg:px-6 flex items-center gap-1 overflow-x-auto">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = currentView === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setCurrentView(item.id)}
              className={`px-3.5 py-2.5 text-xs font-semibold border-b-2 flex items-center gap-2 transition-colors ${
                isActive
                  ? 'border-blue-500 text-white font-bold'
                  : 'border-transparent text-slate-400 hover:text-slate-200 hover:border-slate-700'
              }`}
            >
              <Icon className={`w-3.5 h-3.5 ${isActive ? 'text-blue-400' : 'text-slate-500'}`} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>
    </header>
  );
}
