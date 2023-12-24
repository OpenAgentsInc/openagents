import { useRef } from 'react';
import { useSprings, animated, config } from '@react-spring/web';
import { useDrag } from '@use-gesture/react';
import clamp from 'lodash.clamp';
import swap from 'lodash-move';

import styles from './styles.module.css';

const fn = (order, active = false, originalIndex = 0, curIndex = 0, y = 0) => (index) =>
  active && index === originalIndex
    ? {
      y: curIndex * 100 + y,
      scale: 1.1,
      zIndex: 1,
      shadow: 15,
      immediate: (key) => key === 'zIndex',
      config: (key) => (key === 'y' ? config.stiff : config.default),
    }
    : {
      y: order.indexOf(index) * 100,
      scale: 1,
      zIndex: 0,
      shadow: 1,
      immediate: false,
    };

export function DraggableList({ items }) {
  const order = useRef(items.map((_, index) => index)); // Store indices as a local ref, this represents the item order
  const [springs, api] = useSprings(items.length, fn(order.current)); // Create springs, each corresponds to an item
  const bind = useDrag(({ args: [originalIndex], active, movement: [, y] }) => {
    const curIndex = order.current.indexOf(originalIndex);
    const curRow = clamp(Math.round((curIndex * 100 + y) / 100), 0, items.length - 1);
    const newOrder = swap(order.current, curIndex, curRow);
    api.start(fn(newOrder, active, originalIndex, curIndex, y));
    if (!active) order.current = newOrder;
  });

  return (
    <div className={styles.content} style={{ height: items.length * 100 }}>
      {springs.map(({ zIndex, shadow, y, scale }, i) => (
        <animated.div
          {...bind(i)}
          key={i}
          style={{
            zIndex,
            boxShadow: shadow.to((s) => `rgba(0, 0, 0, 0.15) 0px ${s}px ${2 * s}px 0px`),
            y,
            scale,
          }}
        >
          {items[i]}
        </animated.div>
      ))}
    </div>
  );
}
