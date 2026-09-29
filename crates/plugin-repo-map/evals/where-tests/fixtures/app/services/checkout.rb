class Checkout
  def initialize(cart) = @cart = cart
  def charge! = @cart.total
end
