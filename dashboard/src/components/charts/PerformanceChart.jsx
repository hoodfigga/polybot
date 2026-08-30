import React, { useRef, useEffect, useState } from 'react';

export default function PerformanceChart({
  history = [],
  currentTotal = 40971.60,
  initialCapital = 300.00,
  realizedPnl = 40671.60,
  roiPct = 13557.2
}) {
  const canvasRef = useRef(null);
  const [timeframe, setTimeframe] = useState('1D');
  const [hoverIndex, setHoverIndex] = useState(null);

  const timeframes = ['1H', '4H', '1D', '1W', 'ALL'];

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();

    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const width = rect.width;
    const height = rect.height;

    ctx.clearRect(0, 0, width, height);

    if (history.length < 2) return;

    const padTop = 15;
    const padBottom = 25;
    const padX = 15;
    const plotW = width - padX * 2;
    const plotH = height - padTop - padBottom;

    const values = history.map(d => d.equity);
    const minVal = Math.min(...values) * 0.99;
    const maxVal = Math.max(...values) * 1.01;
    const range = maxVal - minVal || 1;

    const points = history.map((d, i) => {
      const x = padX + (i / (history.length - 1)) * plotW;
      const y = padTop + plotH - ((d.equity - minVal) / range) * plotH;
      return { x, y, ...d };
    });

    // Draw Subtle Grid
    ctx.strokeStyle = 'rgba(26, 37, 56, 0.7)';
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 3]);
    const gridSteps = 3;
    for (let i = 0; i <= gridSteps; i++) {
      const y = padTop + (plotH / gridSteps) * i;
      ctx.beginPath();
      ctx.moveTo(padX, y);
      ctx.lineTo(width - padX, y);
      ctx.stroke();
    }
    ctx.setLineDash([]);

    // Gradient Area Fill
    const gradient = ctx.createLinearGradient(0, padTop, 0, padTop + plotH);
    gradient.addColorStop(0, 'rgba(16, 185, 129, 0.2)');
    gradient.addColorStop(1, 'rgba(16, 185, 129, 0.0)');

    ctx.beginPath();
    ctx.moveTo(points[0].x, padTop + plotH);
    points.forEach((p, idx) => {
      if (idx === 0) ctx.lineTo(p.x, p.y);
      else {
        const prev = points[idx - 1];
        const cx = (prev.x + p.x) / 2;
        ctx.bezierCurveTo(cx, prev.y, cx, p.y, p.x, p.y);
      }
    });
    ctx.lineTo(points[points.length - 1].x, padTop + plotH);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // Curve Stroke
    ctx.beginPath();
    points.forEach((p, idx) => {
      if (idx === 0) ctx.moveTo(p.x, p.y);
      else {
        const prev = points[idx - 1];
        const cx = (prev.x + p.x) / 2;
        ctx.bezierCurveTo(cx, prev.y, cx, p.y, p.x, p.y);
      }
    });
    ctx.strokeStyle = '#10b981';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Pulse dot on last point
    const lastP = points[points.length - 1];
    ctx.beginPath();
    ctx.arc(lastP.x, lastP.y, 4, 0, Math.PI * 2);
    ctx.fillStyle = '#10b981';
    ctx.fill();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Hover line and tooltip indicator
    if (hoverIndex !== null && points[hoverIndex]) {
      const hp = points[hoverIndex];
      ctx.beginPath();
      ctx.setLineDash([2, 2]);
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.moveTo(hp.x, padTop);
      ctx.lineTo(hp.x, padTop + plotH);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.beginPath();
      ctx.arc(hp.x, hp.y, 5, 0, Math.PI * 2);
      ctx.fillStyle = '#ffffff';
      ctx.fill();
      ctx.strokeStyle = '#10b981';
      ctx.lineWidth = 2;
      ctx.stroke();
    }
  }, [history, hoverIndex]);

  const handleMouseMove = (e) => {
    const canvas = canvasRef.current;
    if (!canvas || history.length < 2) return;
    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;

    const padX = 15;
    const plotW = rect.width - padX * 2;
    const relX = Math.max(0, Math.min(plotW, mouseX - padX));
    const idx = Math.round((relX / plotW) * (history.length - 1));
    setHoverIndex(idx);
  };

  const hoveredData = hoverIndex !== null && history[hoverIndex] ? history[hoverIndex] : null;
  const displayEquity = hoveredData ? hoveredData.equity : currentTotal;
  const displayPnl = hoveredData ? hoveredData.equity - initialCapital : realizedPnl;
  const displayPct = (displayPnl / initialCapital) * 100;

  return (
    <div className="card">
      <div className="card-header">
        <div>
          <div className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
            FLEET AGGREGATE EQUITY CURVE
          </div>
          <div className="flex items-baseline gap-2 mt-0.5">
            <span className="text-2xl font-extrabold font-mono text-slate-100">
              ${displayEquity.toLocaleString('en-US', { minimumFractionDigits: 2 })}
            </span>
            <span className="text-xs font-bold font-mono text-emerald-400">
              +{displayPct.toFixed(1)}% (+${displayPnl.toLocaleString('en-US', { minimumFractionDigits: 2 })})
            </span>
            {hoveredData && (
              <span className="text-[11px] font-mono text-slate-500">
                @ {hoveredData.time}
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-1 bg-[#0a0f1a] p-1 rounded-md border border-[#1a2538]">
          {timeframes.map((tf) => (
            <button
              key={tf}
              onClick={() => setTimeframe(tf)}
              className={`px-2.5 py-1 text-[11px] font-semibold rounded transition-colors ${
                timeframe === tf
                  ? 'bg-blue-600 text-white font-bold'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              {tf}
            </button>
          ))}
        </div>
      </div>

      <div
        className="w-full h-48 relative cursor-crosshair px-2 pb-2"
        onMouseMove={handleMouseMove}
        onMouseLeave={() => setHoverIndex(null)}
      >
        <canvas ref={canvasRef} className="w-full h-full block" />
      </div>
    </div>
  );
}
