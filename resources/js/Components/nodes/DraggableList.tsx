import { useRef, useEffect, useState } from 'react';
import { useSprings, animated, config } from '@react-spring/web';
import { useDrag } from '@use-gesture/react';
import clamp from 'lodash.clamp';
import swap from 'lodash-move';

import styles from './styles.module.css';

// This function now takes an additional argument `heights` which is an array of item heights
const fn = (order, heights, active = false, originalIndex = 0, curIndex = 0, y = 0) => (index) => {
  // Calculate the y-offset based on the actual heights of previous items
  const offsetY = order.slice(0, index).reduce((acc, curr) => acc + heights[curr], 0);

  return active && index === originalIndex
    ? {
      y: offsetY + y,
      scale: 1.1,
      zIndex: 1,
      shadow: 15,
      immediate: (key) => key === 'zIndex',
      config: (key) => (key === 'y' ? config.stiff : config.default),
    }
    : {
      y: offsetY,
      scale: 1,
      zIndex: 0,
      shadow: 1,
      immediate: false,
    };
};

export function DraggableList({ items }) {
  const order = useRef(items.map((_, index) => index));
  const itemHeights = useRef(items.map(() => 0)); // Initial heights are set to 0
  const [springs, api] = useSprings(items.length, fn(order.current, itemHeights.current)); // Pass itemHeights.current to fn

  const bind = useDrag(({ args: [originalIndex], active, movement: [, y] }) => {
    const curIndex = order.current.indexOf(originalIndex);
    const curY = itemHeights.current.slice(0, curIndex).reduce((acc, height) => acc + height, 0) + y;
    const curRow = clamp(Math.round(curY / (itemHeights.current[originalIndex] || 100)), 0, items.length - 1);
    const newOrder = swap(order.current, curIndex, curRow);

    // Pass the potentially updated itemHeights.current to fn
    api.start(fn(newOrder, itemHeights.current, active, originalIndex, curIndex, y));

    if (!active) {
      order.current = newOrder;
      // Re-calculate the springs because the order has changed
      api.start(fn(order.current, itemHeights.current));
    }
  });

  useEffect(() => {
    // Trigger a re-render so that the measured heights can be used in the springs
    api.start(fn(order.current, itemHeights.current));
  }, [items.length]); // Dependency on the length of items

  return (
    <div className={styles.content}>
      {springs.map(({ zIndex, shadow, y, scale }, i) => (
        <animated.div
          {...bind(i)}
          key={i}
          ref={el => {
            if (el) itemHeights.current[i] = el.offsetHeight; // Update the height for each item
          }}
          style={{
            zIndex,
            boxShadow: shadow.to(s => `rgba(0, 0, 0, 0.15) 0px ${s}px ${2 * s}px 0px`),
            y,
            scale,
          }}
          children={items[i]}
        />
      ))}
    </div>
  );
}
