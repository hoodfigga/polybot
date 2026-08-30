import React, { useRef, useEffect } from 'react';

export default function MiniDepthChart({ bids = [], asks = [] }) {
  const canvasRef = useRef(null);

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

    if (bids.length === 0 || asks.length === 0) return;

    let cumBid = 0;
    const cumBids = [...bids].sort((a, b) => b.price - a.price).map(b => {
      cumBid += b.size;
      return { price: b.price, cumSize: cumBid };
    }).reverse();

    let cumAsk = 0;
    const cumAsks = [...asks].sort((a, b) => a.price - b.price).map(a => {
      cumAsk += a.size;
      return { price: a.price, cumSize: cumAsk };
    });

    const maxCum = Math.max(cumBid, cumAsk) * 1.1 || 1000;
    const minP = Math.min(bids[bids.length - 1]?.price || 0.40, 0.40);
    const maxP = Math.max(asks[asks.length - 1]?.price || 0.60, 0.60);
    const rangeP = maxP - minP || 0.2;

    const priceToX = (p) => ((p - minP) / rangeP) * width;
    const cumToY = (c) => height - (c / maxCum) * (height - 6);

    // Green Bids Area
    ctx.beginPath();
    ctx.moveTo(priceToX(cumBids[0].price), height);
    cumBids.forEach(b => ctx.lineTo(priceToX(b.price), cumToY(b.cumSize)));
    ctx.lineTo(priceToX(cumBids[cumBids.length - 1].price), height);
    ctx.closePath();
    ctx.fillStyle = 'rgba(16, 185, 129, 0.15)';
    ctx.fill();
    ctx.strokeStyle = '#10b981';
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // Red Asks Area
    ctx.beginPath();
    ctx.moveTo(priceToX(cumAsks[0].price), height);
    cumAsks.forEach(a => ctx.lineTo(priceToX(a.price), cumToY(a.cumSize)));
    ctx.lineTo(priceToX(cumAsks[cumAsks.length - 1].price), height);
    ctx.closePath();
    ctx.fillStyle = 'rgba(244, 63, 94, 0.15)';
    ctx.fill();
    ctx.strokeStyle = '#f43f5e';
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }, [bids, asks]);

  return (
    <div className="w-full h-16 relative">
      <canvas ref={canvasRef} className="w-full h-full block" />
    </div>
  );
}
