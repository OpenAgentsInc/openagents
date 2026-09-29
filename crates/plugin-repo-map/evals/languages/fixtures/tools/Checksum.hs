module Checksum (checksum) where

-- | A simple additive checksum over bytes.
checksum :: [Int] -> Int
checksum = foldr (\b acc -> (acc + b) `mod` 65521) 1
