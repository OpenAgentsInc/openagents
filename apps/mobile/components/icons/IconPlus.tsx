import { AntDesign } from '@expo/vector-icons'
import { DARK_THEME } from '../../constants/colors'

interface IconPlusProps {
  size?: number
  color?: string
}

export const IconPlus = ({ size = 20, color = DARK_THEME.text }: IconPlusProps) => {
  return <AntDesign name="plus" size={size} color={color} />
}