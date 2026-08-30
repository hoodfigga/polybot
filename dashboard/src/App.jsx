import React from 'react';
import { TradingProvider, useTrading } from './context/TradingContext';
import Header from './components/common/Header';
import OverviewView from './components/views/OverviewView';
import MarketsView from './components/views/MarketsView';
import PositionsView from './components/views/PositionsView';
import FleetView from './components/views/FleetView';
import SignalsView from './components/views/SignalsView';

function DashboardContent() {
  const { currentView } = useTrading();

  return (
    <div className="min-h-screen flex flex-col bg-[#080c14] text-slate-100 font-sans selection:bg-blue-600/30">
      <Header />

      <main className="flex-1 max-w-7xl w-full mx-auto p-4 sm:p-6 lg:p-8">
        {currentView === 'overview' && <OverviewView />}
        {currentView === 'markets' && <MarketsView />}
        {currentView === 'positions' && <PositionsView />}
        {currentView === 'fleet' && <FleetView />}
        {currentView === 'signals' && <SignalsView />}
      </main>

      <footer className="border-t border-[#1a2538] bg-[#0b101c] py-3.5 px-4 sm:px-6 text-[11px] text-slate-500 font-mono flex flex-col sm:flex-row items-center justify-between gap-2">
        <div className="flex items-center gap-3">
          <span className="text-slate-400 font-semibold">Tabula Trader Engine v3.0</span>
          <span>•</span>
          <span>Settlement: <strong>Polygon PoS (Chain ID 137)</strong></span>
          <span>•</span>
          <span>Signing: <strong>EIP-712 Typed Data</strong></span>
        </div>
        <div>
          AWS Swarm: <strong>6 Active Production Nodes</strong>
        </div>
      </footer>
    </div>
  );
}

export default function App() {
  return (
    <TradingProvider>
      <DashboardContent />
    </TradingProvider>
  );
}
