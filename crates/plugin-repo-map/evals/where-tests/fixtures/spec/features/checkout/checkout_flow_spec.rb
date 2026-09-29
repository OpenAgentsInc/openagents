require "spec_helper"

RSpec.describe Checkout do
  it "charges the cart total" do
    cart = Cart.new
    expect(Checkout.new(cart).charge!).to eq(0)
  end
end
