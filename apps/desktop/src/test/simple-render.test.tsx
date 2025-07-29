import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';

function SimpleComponent() {
  return <div data-testid="simple">Hello World</div>;
}

describe('Simple Render Test', () => {
  it('should render a simple component', () => {
    const { getByTestId } = render(<SimpleComponent />);
    expect(getByTestId('simple')).toBeTruthy();
  });
});