require "spec_helper"

RSpec.describe Cart do
  it "starts empty" do
    expect(Cart.new.total).to eq(0)
  end
end
