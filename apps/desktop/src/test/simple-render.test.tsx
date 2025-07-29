import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { act } from 'react';
import React from 'react';

function SimpleComponent() {
  return <div data-testid="simple">Hello World</div>;
}

describe('Simple Render Test', () => {
  it('should render a simple component', async () => {
    let renderResult;
    await act(async () => {
      renderResult = render(<SimpleComponent />);
    });
    expect(renderResult!.getByTestId('simple')).toBeTruthy();
  });
});