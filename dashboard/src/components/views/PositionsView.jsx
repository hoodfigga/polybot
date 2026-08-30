import React from 'react';
import { useTrading } from '../../context/TradingContext';
import DataTable from '../common/DataTable';
import StatusBadge from '../common/StatusBadge';
import { Layers, History, ShieldCheck, ArrowUpRight, ArrowDownRight, Check } from 'lucide-react';

export default function PositionsView() {
  const { positions, recentTrades, closePosition } = useTrading();

  const positionColumns = [
    {
      header: 'ID',
      accessor: 'id',
      width: '90px',
      render: (_, val) => <span className="font-mono text-slate-400 text-[11px]">{val}</span>
    },
    {
      header: 'Market Event',
      accessor: 'marketTitle',
      render: (row) => (
        <div>
          <div className="font-semibold text-slate-100">{row.marketTitle}</div>
          <div className="text-[10px] font-mono text-slate-500">{row.conditionId}</div>
        </div>
      )
    },
    {
      header: 'Strategy Profile',
      accessor: 'strategy',
      width: '160px',
      render: (_, val) => (
        <StatusBadge 
          status={val} 
          variant={val.includes('PARITY') ? 'emerald' : val.includes('MM') ? 'blue' : 'purple'} 
          size="sm" 
        />
      )
    },
    {
      header: 'Contracts & Distribution',
      accessor: 'holdings',
      render: (_, val) => <span className="font-mono text-slate-300 text-[11px]">{val}</span>
    },
    {
      header: 'Notional',
      accessor: 'notionalUsd',
      align: 'right',
      width: '110px',
      render: (_, val) => <span className="font-mono font-bold text-slate-100">${val.toFixed(2)}</span>
    },
    {
      header: 'Unrealized P&L',
      accessor: 'unrealizedPnl',
      align: 'right',
      width: '130px',
      render: (row) => {
        const isPos = row.unrealizedPnl >= 0;
        return (
          <div className={`font-mono font-bold flex items-center justify-end gap-0.5 ${isPos ? 'text-emerald-400' : 'text-rose-400'}`}>
            {isPos ? <ArrowUpRight className="w-3.5 h-3.5" /> : <ArrowDownRight className="w-3.5 h-3.5" />}
            <span>{isPos ? '+' : ''}${row.unrealizedPnl.toFixed(2)} ({isPos ? '+' : ''}{row.unrealizedPnlPct.toFixed(1)}%)</span>
          </div>
        );
      }
    },
    {
      header: 'Hedge Status',
      accessor: 'hedgeStatus',
      width: '170px',
      render: (_, val) => (
        <span className="inline-flex items-center gap-1 text-[11px] font-semibold text-emerald-400">
          <ShieldCheck className="w-3.5 h-3.5" />
          <span>{val}</span>
        </span>
      )
    },
    {
      header: 'Action',
      align: 'right',
      width: '100px',
      render: (row) => (
        <button
          onClick={() => closePosition(row.id)}
          className="btn btn-sm text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-200 border-slate-700"
        >
          Unwind
        </button>
      )
    }
  ];

  const tradeColumns = [
    {
      header: 'Timestamp',
      accessor: 'timestamp',
      width: '100px',
      render: (_, val) => <span className="font-mono text-slate-400 text-[11px]">{val}</span>
    },
    {
      header: 'Ticket ID',
      accessor: 'id',
      width: '90px',
      render: (_, val) => <span className="font-mono font-bold text-slate-200 text-[11px]">{val}</span>
    },
    {
      header: 'Executing Node',
      accessor: 'node',
      width: '190px',
      render: (_, val) => <span className="font-mono text-slate-300 text-[11px]">{val}</span>
    },
    {
      header: 'Target Market',
      accessor: 'market',
      render: (_, val) => <span className="font-medium text-slate-100">{val}</span>
    },
    {
      header: 'Order Execution Type',
      accessor: 'type',
      width: '150px',
      render: (_, val) => (
        <StatusBadge 
          status={val} 
          variant={val.includes('BASKET') ? 'emerald' : val.includes('TAKER') ? 'cyan' : 'blue'} 
          size="sm" 
        />
      )
    },
    {
      header: 'Size (USDC)',
      accessor: 'amountUsd',
      align: 'right',
      width: '110px',
      render: (_, val) => <span className="font-mono font-bold text-slate-200">${val.toFixed(2)}</span>
    },
    {
      header: 'Realized Profit',
      accessor: 'netProfit',
      align: 'right',
      width: '120px',
      render: (_, val) => (
        <span className="font-mono font-bold text-emerald-400">
          +${val.toFixed(2)}
        </span>
      )
    },
    {
      header: 'Settlement',
      accessor: 'status',
      width: '100px',
      render: (_, val) => (
        <span className="inline-flex items-center gap-1 text-[10px] font-semibold text-emerald-400">
          <Check className="w-3 h-3" />
          <span>{val}</span>
        </span>
      )
    }
  ];

  return (
    <div className="space-y-6">
      {/* Active Positions */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-sm font-bold text-slate-100 uppercase tracking-wider flex items-center gap-2">
              <Layers className="w-4 h-4 text-blue-400" />
              <span>Active Portfolio Positions ({positions.length})</span>
            </h2>
            <p className="text-xs text-slate-400 mt-0.5">Prediction contracts currently held and hedged across CLOB books</p>
          </div>
        </div>

        <DataTable
          columns={positionColumns}
          data={positions}
          emptyMessage="No active positions open. The engine is scanning for parity mispricings."
        />
      </div>

      {/* Execution Audit Log */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-sm font-bold text-slate-100 uppercase tracking-wider flex items-center gap-2">
              <History className="w-4 h-4 text-slate-400" />
              <span>Swarm Execution Audit Log ({recentTrades.length})</span>
            </h2>
            <p className="text-xs text-slate-400 mt-0.5">Complete chronological record of all settled trade tickets</p>
          </div>
        </div>

        <DataTable
          columns={tradeColumns}
          data={recentTrades}
          emptyMessage="No trade history available."
        />
      </div>
    </div>
  );
}
