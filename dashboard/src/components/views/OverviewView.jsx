import React from 'react';
import { useTrading } from '../../context/TradingContext';
import StatCard from '../common/StatCard';
import StatusBadge from '../common/StatusBadge';
import PerformanceChart from '../charts/PerformanceChart';
import DataTable from '../common/DataTable';
import { 
  DollarSign, 
  TrendingUp, 
  ShieldCheck, 
  Server, 
  Zap, 
  ArrowUpRight, 
  Target,
  Clock,
  CheckCircle2
} from 'lucide-react';

export default function OverviewView() {
  const { 
    engineState, 
    equityHistory, 
    markets, 
    recentTrades, 
    fleetNodes,
    executeManualOrder,
    setCurrentView,
    setSelectedMarketId
  } = useTrading();

  // Filter parity opportunities
  const arbOpportunities = markets.filter(m => m.isArbOpportunity);

  const tradeColumns = [
    {
      header: 'Time',
      accessor: 'timestamp',
      width: '90px',
      render: (_, val) => <span className="font-mono text-slate-400 text-[11px]">{val}</span>
    },
    {
      header: 'Node Source',
      accessor: 'node',
      width: '170px',
      render: (_, val) => <span className="font-mono text-slate-300 font-medium text-[11px]">{val}</span>
    },
    {
      header: 'Market Event',
      accessor: 'market',
      render: (_, val) => <span className="font-medium text-slate-100">{val}</span>
    },
    {
      header: 'Strategy Type',
      accessor: 'type',
      width: '140px',
      render: (_, val) => (
        <StatusBadge 
          status={val} 
          variant={val.includes('BASKET') ? 'emerald' : val.includes('TAKER') ? 'cyan' : 'blue'} 
          size="sm" 
        />
      )
    },
    {
      header: 'Notional',
      accessor: 'amountUsd',
      align: 'right',
      width: '100px',
      render: (_, val) => <span className="font-mono font-bold text-slate-200">${val.toFixed(2)}</span>
    },
    {
      header: 'Net Return',
      accessor: 'netProfit',
      align: 'right',
      width: '110px',
      render: (_, val) => (
        <span className="font-mono font-bold text-emerald-400">
          +${val.toFixed(2)}
        </span>
      )
    }
  ];

  return (
    <div className="space-y-6">
      {/* 4 Primary Executive Stat Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          label="Total Fleet Capital"
          value={`$${engineState.totalEquity.toLocaleString('en-US', { minimumFractionDigits: 2 })}`}
          trend={`+${engineState.roiPct.toFixed(1)}%`}
          trendPositive={true}
          subtitle="All-time ROIC"
          badge="USDC.e"
          icon={DollarSign}
        />

        <StatCard
          label="Realized Net Profit"
          value={`+$${engineState.realizedPnl.toLocaleString('en-US', { minimumFractionDigits: 2 })}`}
          subtitle={`On $${engineState.initialCapital.toFixed(2)} Initial Seed`}
          badge="0 Drawdown"
          icon={TrendingUp}
        />

        <StatCard
          label="Execution Win Rate"
          value={`${engineState.winRate.toFixed(1)}%`}
          subtitle={`Sharpe Ratio: ${engineState.sharpeRatio.toFixed(1)}`}
          badge="Mathematical Parity"
          icon={ShieldCheck}
        />

        <StatCard
          label="AWS Distributed Fleet"
          value={`${engineState.activeNodesCount} Active Nodes`}
          subtitle={`Avg RTT Latency: ${engineState.avgLatencyMs}ms`}
          badge="us-east-1 + ap-south-1"
          icon={Server}
        />
      </div>

      {/* Main Performance Chart */}
      <PerformanceChart
        history={equityHistory}
        currentTotal={engineState.totalEquity}
        initialCapital={engineState.initialCapital}
        realizedPnl={engineState.realizedPnl}
        roiPct={engineState.roiPct}
      />

      {/* Grid: Live Arbitrage Scanner & Quick Swarm Health */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left 2 Cols: Live Parity Opportunities */}
        <div className="lg:col-span-2 card">
          <div className="card-header">
            <div className="flex items-center gap-2">
              <Target className="w-4 h-4 text-emerald-400" />
              <span className="font-semibold text-slate-200 text-xs uppercase tracking-wider">
                Active Dutch-Book Parity Arbitrage Opportunities (Sum &lt; $0.985)
              </span>
            </div>
            <button
              onClick={() => setCurrentView('markets')}
              className="text-[11px] font-semibold text-blue-400 hover:text-blue-300 transition-colors"
            >
              View Order Books →
            </button>
          </div>

          <div className="card-body p-0 divide-y divide-[#1a2538]">
            {arbOpportunities.map((opp) => (
              <div key={opp.id} className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3 hover:bg-[#111928] transition-colors">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-slate-800 text-slate-300 border border-slate-700">
                      {opp.category}
                    </span>
                    <h4 className="font-semibold text-slate-100 text-xs">{opp.title}</h4>
                  </div>
                  <div className="text-[11px] text-slate-400 flex items-center gap-3 font-mono">
                    <span>YES: <strong>${opp.yesPrice.toFixed(3)}</strong></span>
                    <span>NO: <strong>${opp.noPrice.toFixed(3)}</strong></span>
                    <span>Sibling Cost: <strong className="text-amber-400">${opp.siblingSum.toFixed(3)}</strong></span>
                  </div>
                </div>

                <div className="flex items-center gap-3 self-end sm:self-center">
                  <div className="text-right">
                    <span className="text-[10px] text-slate-500 uppercase block font-semibold">Net Yield</span>
                    <span className="text-xs font-bold font-mono text-emerald-400">+{opp.netMarginPct.toFixed(2)}%</span>
                  </div>
                  <button
                    onClick={() => {
                      executeManualOrder({ type: 'ATOMIC_BASKET', amountUsd: 40.00 });
                    }}
                    className="btn btn-sm btn-emerald"
                  >
                    Execute Basket ($40)
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Right 1 Col: AWS Fleet Quick Status */}
        <div className="card flex flex-col justify-between">
          <div>
            <div className="card-header">
              <div className="flex items-center gap-2">
                <Server className="w-4 h-4 text-cyan-400" />
                <span className="font-semibold text-slate-200 text-xs uppercase tracking-wider">
                  Swarm Health ({fleetNodes.length} Nodes)
                </span>
              </div>
              <button
                onClick={() => setCurrentView('fleet')}
                className="text-[11px] font-semibold text-blue-400 hover:text-blue-300 transition-colors"
              >
                Manage Fleet →
              </button>
            </div>

            <div className="card-body p-3 space-y-2">
              {fleetNodes.slice(0, 4).map((node) => (
                <div key={node.id} className="p-2 rounded bg-[#0a0f1a] border border-[#1a2538] flex items-center justify-between text-xs">
                  <div>
                    <div className="font-semibold text-slate-200 text-[11px]">{node.name}</div>
                    <div className="text-[10px] text-slate-500 font-mono">{node.strategy} • {node.latencyMs}ms RTT</div>
                  </div>
                  <div className="text-right font-mono">
                    <div className="font-bold text-slate-100 text-[11px]">${node.currentEquity.toFixed(0)}</div>
                    <div className="text-[10px] text-emerald-400 font-semibold">+${node.pnl.toFixed(0)}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="p-3 bg-[#0a0f1a] border-t border-[#1a2538] flex items-center justify-between text-[11px] text-slate-400">
            <span>Last Federated Sync:</span>
            <strong className="font-mono text-slate-200">{engineState.lastSyncTime}</strong>
          </div>
        </div>
      </div>

      {/* Recent Executions Audit Trail */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <h3 className="font-bold text-slate-200 text-xs uppercase tracking-wider flex items-center gap-2">
            <Clock className="w-4 h-4 text-slate-400" />
            <span>Recent Swarm Order Executions & Settlements</span>
          </h3>
          <button
            onClick={() => setCurrentView('positions')}
            className="text-[11px] font-semibold text-blue-400 hover:text-blue-300 transition-colors"
          >
            Full Trade Audit Log →
          </button>
        </div>

        <DataTable
          columns={tradeColumns}
          data={recentTrades.slice(0, 5)}
          emptyMessage="No recent trades recorded."
        />
      </div>
    </div>
  );
}
