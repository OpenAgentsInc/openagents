class Cart
  def initialize = @items = []
  def add(item) = @items << item
  def total = @items.sum(&:price)
end
