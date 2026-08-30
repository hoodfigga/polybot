import React, { useState } from 'react';
import { useTrading } from '../../context/TradingContext';
import StatusBadge from '../common/StatusBadge';
import MiniDepthChart from '../charts/MiniDepthChart';
import { 
  TrendingUp, 
  Search, 
  AlignJustify, 
  Zap, 
  Send, 
  ShieldCheck, 
  Sliders,
  DollarSign,
  Activity
} from 'lucide-react';

export default function MarketsView() {
  const { 
    markets, 
    selectedMarketId, 
    setSelectedMarketId, 
    activeMarket, 
    executeManualOrder 
  } = useTrading();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState('ALL');
  const [orderSide, setOrderSide] = useState('BASKET'); // 'YES' | 'NO' | 'BASKET'
  const [orderSizeUsd, setOrderSizeUsd] = useState(40.00);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const categories = ['ALL', 'Politics', 'Fed & Rates', 'Macro Economics', 'Crypto 5M'];

  const filteredMarkets = markets.filter(m => {
    const matchesCat = selectedCategory === 'ALL' || m.category.toLowerCase().includes(selectedCategory.toLowerCase());
    const matchesSearch = m.title.toLowerCase().includes(searchQuery.toLowerCase()) || m.conditionId.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesCat && matchesSearch;
  });

  const price = orderSide === 'YES' ? activeMarket.yesPrice : orderSide === 'NO' ? activeMarket.noPrice : activeMarket.siblingSum;
  const contracts = price > 0 ? Math.floor(orderSizeUsd / price) : 0;
  const payout = contracts * 1.0;
  const netProfit = payout - orderSizeUsd;
  const roicPct = orderSizeUsd > 0 ? (netProfit / orderSizeUsd) * 100 : 0;

  const handleOrderSubmit = (e) => {
    e.preventDefault();
    setIsSubmitting(true);
    setTimeout(() => {
      executeManualOrder({
        type: orderSide === 'BASKET' ? 'ATOMIC_BASKET' : `TAKER_${orderSide}`,
        amountUsd: orderSizeUsd,
        contracts,
        price
      });
      setIsSubmitting(false);
    }, 250);
  };

  const maxBidSize = Math.max(...activeMarket.bids.map(b => b.size), 1);
  const maxAskSize = Math.max(...activeMarket.asks.map(a => a.size), 1);

  return (
    <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
      {/* Left Column: Market Explorer (5 cols) */}
      <div className="lg:col-span-5 space-y-4">
        {/* Search & Category Bar */}
        <div className="card p-3 space-y-3">
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-slate-500 absolute left-3 top-2.5" />
            <input
              type="text"
              placeholder="Search markets or condition ID..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="input pl-8"
            />
          </div>

          <div className="flex items-center gap-1.5 overflow-x-auto pb-1">
            {categories.map((cat) => (
              <button
                key={cat}
                onClick={() => setSelectedCategory(cat)}
                className={`px-2.5 py-1 rounded text-[11px] font-semibold transition-colors whitespace-nowrap ${
                  selectedCategory === cat
                    ? 'bg-blue-600 text-white font-bold'
                    : 'bg-[#0a0f1a] text-slate-400 hover:text-slate-200 border border-[#1a2538]'
                }`}
              >
                {cat}
              </button>
            ))}
          </div>
        </div>

        {/* Markets List */}
        <div className="space-y-2">
          {filteredMarkets.map((m) => {
            const isSelected = m.id === selectedMarketId;
            return (
              <div
                key={m.id}
                onClick={() => setSelectedMarketId(m.id)}
                className={`card p-3 cursor-pointer transition-all ${
                  isSelected
                    ? 'border-blue-500 bg-[#111928]'
                    : 'hover:border-slate-700 hover:bg-[#0d1320]'
                }`}
              >
                <div className="flex items-start justify-between gap-2 mb-1.5">
                  <div className="space-y-0.5">
                    <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-slate-800 text-slate-400 border border-slate-700">
                      {m.category}
                    </span>
                    <h4 className="font-semibold text-slate-100 text-xs leading-snug mt-1">{m.title}</h4>
                  </div>
                  {m.isArbOpportunity && (
                    <StatusBadge status={`+${m.netMarginPct.toFixed(1)}% ARB`} variant="emerald" size="sm" />
                  )}
                </div>

                <div className="flex items-center justify-between pt-2 border-t border-[#1a2538] text-[11px] font-mono text-slate-400">
                  <div className="flex items-center gap-2">
                    <span>YES: <strong className="text-emerald-400">${m.yesPrice.toFixed(2)}</strong></span>
                    <span>NO: <strong className="text-rose-400">${m.noPrice.toFixed(2)}</strong></span>
                  </div>
                  <span>Vol: ${(m.volume24h / 1000).toFixed(0)}k</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Right Column: Selected Market Detail & Execution (7 cols) */}
      <div className="lg:col-span-7 space-y-4">
        {/* Market Title & Live Metrics Header */}
        <div className="card p-4 space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <StatusBadge status={activeMarket.category} variant="blue" size="sm" />
                <span className="text-[11px] font-mono text-slate-500">{activeMarket.conditionId}</span>
              </div>
              <h2 className="text-base font-bold text-slate-100">{activeMarket.title}</h2>
            </div>
            <div className="text-right font-mono">
              <div className="text-lg font-black text-slate-100">${activeMarket.yesPrice.toFixed(3)}</div>
              <div className="text-[10px] text-slate-500">Polygon PoS CLOB</div>
            </div>
          </div>

          <div className="grid grid-cols-4 gap-2 pt-3 border-t border-[#1a2538] text-[11px] font-mono">
            <div className="bg-[#0a0f1a] p-2 rounded border border-[#1a2538]">
              <span className="text-slate-500 block text-[10px]">SPREAD</span>
              <strong className="text-slate-200">${activeMarket.spread.toFixed(3)}</strong>
            </div>
            <div className="bg-[#0a0f1a] p-2 rounded border border-[#1a2538]">
              <span className="text-slate-500 block text-[10px]">OFI DRIFT</span>
              <strong className={activeMarket.ofi >= 0 ? 'text-emerald-400' : 'text-rose-400'}>
                {activeMarket.ofi >= 0 ? '+' : ''}{activeMarket.ofi.toFixed(2)}
              </strong>
            </div>
            <div className="bg-[#0a0f1a] p-2 rounded border border-[#1a2538]">
              <span className="text-slate-500 block text-[10px]">SIBLING SUM</span>
              <strong className="text-amber-400">${activeMarket.siblingSum.toFixed(3)}</strong>
            </div>
            <div className="bg-[#0a0f1a] p-2 rounded border border-[#1a2538]">
              <span className="text-slate-500 block text-[10px]">24H VOLUME</span>
              <strong className="text-slate-200">${(activeMarket.volume24h / 1000).toFixed(1)}k</strong>
            </div>
          </div>
        </div>

        {/* Level-2 Order Book Depth & Curve */}
        <div className="card">
          <div className="card-header">
            <div className="flex items-center gap-2">
              <AlignJustify className="w-4 h-4 text-cyan-400" />
              <span className="font-semibold text-slate-200 text-xs uppercase tracking-wider">
                Level-2 Order Book Depth (CLOB)
              </span>
            </div>
            <span className="text-[11px] font-mono text-slate-400">Zero Taker Fee</span>
          </div>

          <div className="card-body p-3 space-y-3">
            <MiniDepthChart bids={activeMarket.bids} asks={activeMarket.asks} />

            <div className="grid grid-cols-2 gap-3 font-mono text-xs">
              {/* Bids */}
              <div className="space-y-1">
                <div className="flex justify-between text-[10px] text-slate-500 uppercase pb-1 border-b border-[#1a2538]">
                  <span>Size (Shares)</span>
                  <span>Bid Price</span>
                </div>
                {activeMarket.bids.map((b, i) => (
                  <div key={i} className="relative flex justify-between py-1 px-2 rounded hover:bg-emerald-950/20">
                    <div 
                      className="absolute right-0 top-0 bottom-0 bg-emerald-500/10 rounded" 
                      style={{ width: `${(b.size / maxBidSize) * 100}%` }}
                    />
                    <span className="relative z-10 text-slate-400 text-[11px]">{b.size.toLocaleString()}</span>
                    <span className="relative z-10 font-bold text-emerald-400 text-[11px]">${b.price.toFixed(2)}</span>
                  </div>
                ))}
              </div>

              {/* Asks */}
              <div className="space-y-1">
                <div className="flex justify-between text-[10px] text-slate-500 uppercase pb-1 border-b border-[#1a2333]">
                  <span>Ask Price</span>
                  <span>Size (Shares)</span>
                </div>
                {activeMarket.asks.map((a, i) => (
                  <div key={i} className="relative flex justify-between py-1 px-2 rounded hover:bg-rose-950/20">
                    <div 
                      className="absolute left-0 top-0 bottom-0 bg-rose-500/10 rounded" 
                      style={{ width: `${(a.size / maxAskSize) * 100}%` }}
                    />
                    <span className="relative z-10 font-bold text-rose-400 text-[11px]">${a.price.toFixed(2)}</span>
                    <span className="relative z-10 text-slate-400 text-[11px]">{a.size.toLocaleString()}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Fast Order Execution Form */}
        <div className="card p-4 space-y-4">
          <div className="flex items-center justify-between border-b border-[#1a2538] pb-2">
            <span className="font-semibold text-slate-200 text-xs uppercase tracking-wider flex items-center gap-1.5">
              <Zap className="w-3.5 h-3.5 text-blue-400" />
              <span>Instant Gasless EIP-712 Order Execution</span>
            </span>
            <span className="text-[10px] font-mono text-emerald-400 font-bold">Polygon Chain 137</span>
          </div>

          <form onSubmit={handleOrderSubmit} className="space-y-3">
            {/* Outcome Target */}
            <div className="grid grid-cols-3 gap-2">
              <button
                type="button"
                onClick={() => setOrderSide('YES')}
                className={`py-2 px-3 rounded font-bold text-xs flex flex-col items-center justify-center transition-all ${
                  orderSide === 'YES'
                    ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/50'
                    : 'bg-[#0a0f1a] text-slate-400 border border-[#1a2538] hover:border-slate-700'
                }`}
              >
                <span>BUY YES</span>
                <span className="font-mono text-[10px] text-emerald-400">${activeMarket.yesPrice.toFixed(2)}</span>
              </button>

              <button
                type="button"
                onClick={() => setOrderSide('NO')}
                className={`py-2 px-3 rounded font-bold text-xs flex flex-col items-center justify-center transition-all ${
                  orderSide === 'NO'
                    ? 'bg-rose-500/20 text-rose-300 border border-rose-500/50'
                    : 'bg-[#0a0f1a] text-slate-400 border border-[#1a2538] hover:border-slate-700'
                }`}
              >
                <span>BUY NO</span>
                <span className="font-mono text-[10px] text-rose-400">${activeMarket.noPrice.toFixed(2)}</span>
              </button>

              <button
                type="button"
                onClick={() => setOrderSide('BASKET')}
                className={`py-2 px-3 rounded font-bold text-xs flex flex-col items-center justify-center transition-all ${
                  orderSide === 'BASKET'
                    ? 'bg-blue-600/20 text-blue-300 border border-blue-500/50'
                    : 'bg-[#0a0f1a] text-slate-400 border border-[#1a2538] hover:border-slate-700'
                }`}
              >
                <span>PARITY BASKET</span>
                <span className="font-mono text-[10px] text-blue-400">${activeMarket.siblingSum.toFixed(3)}</span>
              </button>
            </div>

            {/* Sizing presets */}
            <div className="space-y-1.5">
              <div className="flex justify-between text-[11px] text-slate-400">
                <span>NOTIONAL POSITION SIZE</span>
                <span className="font-mono font-bold text-slate-200">${orderSizeUsd.toFixed(2)} USDC</span>
              </div>
              <div className="grid grid-cols-4 gap-1.5">
                {[10, 25, 40, 50].map((sz) => (
                  <button
                    key={sz}
                    type="button"
                    onClick={() => setOrderSizeUsd(sz)}
                    className={`py-1 rounded font-mono text-[11px] font-semibold transition-colors ${
                      orderSizeUsd === sz
                        ? 'bg-blue-600 text-white font-bold'
                        : 'bg-[#0a0f1a] text-slate-400 border border-[#1a2538] hover:text-slate-200'
                    }`}
                  >
                    ${sz}
                  </button>
                ))}
              </div>
            </div>

            {/* Execution Estimate Card */}
            <div className="bg-[#0a0f1a] border border-[#1a2538] rounded p-2.5 space-y-1 text-[11px] font-mono">
              <div className="flex justify-between text-slate-400">
                <span>Estimated Contracts:</span>
                <strong className="text-slate-200">{contracts} Shares</strong>
              </div>
              <div className="flex justify-between text-slate-400">
                <span>Guaranteed Settle Payout:</span>
                <strong className="text-slate-200">${payout.toFixed(2)} USDC</strong>
              </div>
              <div className="flex justify-between text-emerald-400 pt-1 border-t border-[#1a2538] font-bold">
                <span>Net Return Margin:</span>
                <span>+{roicPct.toFixed(2)}% (+${netProfit.toFixed(2)})</span>
              </div>
            </div>

            <button
              type="submit"
              disabled={isSubmitting}
              className={`w-full py-2 rounded font-bold text-xs btn ${
                orderSide === 'BASKET' ? 'btn-primary' : orderSide === 'YES' ? 'btn-emerald' : 'btn-danger'
              }`}
            >
              <Send className="w-3.5 h-3.5" />
              <span>{isSubmitting ? 'Submitting...' : `Dispatch ${orderSide} Order ($${orderSizeUsd.toFixed(2)})`}</span>
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
