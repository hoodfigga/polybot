import React from 'react';

export default function DataTable({
  columns = [],
  data = [],
  emptyMessage = 'No records found.',
  onRowClick
}) {
  return (
    <div className="table-container card">
      <table>
        <thead>
          <tr>
            {columns.map((col, idx) => (
              <th
                key={idx}
                className={col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'}
                style={{ width: col.width }}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="py-8 text-center text-slate-500 text-xs">
                {emptyMessage}
              </td>
            </tr>
          ) : (
            data.map((row, rowIdx) => (
              <tr
                key={row.id || rowIdx}
                onClick={() => onRowClick?.(row)}
                className={onRowClick ? 'cursor-pointer transition-colors' : ''}
              >
                {columns.map((col, colIdx) => {
                  const val = col.accessor ? (typeof col.accessor === 'function' ? col.accessor(row) : row[col.accessor]) : null;
                  return (
                    <td
                      key={colIdx}
                      className={col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'}
                    >
                      {col.render ? col.render(row, val) : val}
                    </td>
                  );
                })}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
